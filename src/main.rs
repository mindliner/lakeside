use std::fs::File;
use std::io::{BufWriter, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use cdk::nuts::nut00::KnownMethod;
use cdk::wallet::Wallet;
use chrono::{Local, Utc};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tower_http::trace::TraceLayer;

const TOKEN_FORMAT_LABEL: &str = "cashu-b";
const DEFAULT_LOWER_BOUND: u64 = 10;
const DEFAULT_UPPER_BOUND: u64 = 20;

use token_amount::{compute_sum_total, compute_token_values, AmountStrategy};
use wallet::{open_wallet, send_and_export_token, LakesideWallet, LakesideWalletType};

use crate::tickets::{
    import_from_csv, init_store, list_summary, normalize_ticket_code, TicketRecord, TicketStatus,
    TicketStore, TokenBundleRecord,
};

mod tickets;
mod token_amount;
mod wallet;

#[derive(Args, Clone, Default)]
struct AmountArgs {
    /// Fixed amount (in sats) for every token
    #[arg(short = 'f', long)]
    fixed_amount: Option<u64>,
    /// Lower bound for random payouts
    #[arg(short = 'l', long)]
    lower_bound: Option<u64>,
    /// Upper bound for random payouts
    #[arg(short = 'u', long)]
    upper_bound: Option<u64>,
}

impl AmountArgs {
    fn resolve(&self) -> Result<AmountStrategy> {
        match (self.fixed_amount, self.lower_bound, self.upper_bound) {
            (Some(fixed), None, None) => {
                if fixed == 0 {
                    bail!("--fixed-amount must be greater than zero");
                }
                Ok(AmountStrategy::Fixed(fixed))
            }
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                bail!(
                    "Provide either --fixed-amount or (--lower-bound AND --upper-bound), not both"
                );
            }
            (None, Some(lower), Some(upper)) => {
                if lower == 0 || upper == 0 {
                    bail!("Bounds must be greater than zero");
                }
                if upper < lower {
                    bail!("--upper-bound must be greater than or equal to --lower-bound");
                }
                Ok(AmountStrategy::Range { lower, upper })
            }
            (None, Some(_), None) | (None, None, Some(_)) => {
                bail!("Specify both --lower-bound and --upper-bound for ranged payouts");
            }
            (None, None, None) => Ok(AmountStrategy::Range {
                lower: DEFAULT_LOWER_BOUND,
                upper: DEFAULT_UPPER_BOUND,
            }),
        }
    }
}

#[derive(Parser)]
#[command(
    name = "lakeside",
    author = "Marius <marius@mountainlake.io>",
    version = "0.1.2",
    about = "Mints Cashu tokens and manages ticket-gated faucet data",
    arg_required_else_help = false
)]
struct Cli {
    #[command(flatten)]
    mint: MintArgs,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Args, Clone)]
struct MintArgs {
    /// URL of the Cashu mint
    #[arg(short, long, default_value = "https://m7.mountainlake.io")]
    mint: String,

    #[command(flatten)]
    amount: AmountArgs,

    /// Number of tokens to mint
    #[arg(short = 'n', long, default_value_t = 4)]
    token_count: u64,

    /// Output filename (tab-separated value + token)
    #[arg(short, long, default_value = "cashu_tokens.txt")]
    output_filename: String,

    /// Use Bolt12 invoices instead of Bolt11
    #[arg(long)]
    bolt12: bool,

    /// Persist the wallet between runs
    #[arg(short, long)]
    persistent_wallet: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the ticket datastore used by the faucet
    #[command(subcommand)]
    Tickets(TicketsCommand),
    /// Run faucet utilities (HTTP server, etc.)
    #[command(subcommand)]
    Faucet(FaucetCommand),
    /// Inspect or manage wallets
    #[command(subcommand)]
    Wallet(WalletCommand),
}

#[derive(Subcommand)]
enum TicketsCommand {
    /// Create an empty tickets file
    Init {
        /// Path to the tickets JSON file
        #[arg(long, default_value = "tickets.json")]
        output: PathBuf,
        /// Overwrite if the file already exists
        #[arg(long)]
        force: bool,
    },
    /// Import ticket codes from a CSV export
    Import {
        /// CSV file containing ticket codes (requires headers)
        #[arg(long)]
        csv: PathBuf,
        /// Tickets JSON file to update
        #[arg(long, default_value = "tickets.json")]
        store: PathBuf,
        /// Column to use as the ticket code
        #[arg(long, default_value = "ticket_code")]
        code_column: String,
        /// Optional metadata columns to copy into the tickets file
        #[arg(long = "metadata-column")]
        metadata_columns: Vec<String>,
        /// Optional delimiter (defaults to ',')
        #[arg(long)]
        delimiter: Option<char>,
        /// Preserve hyphens in ticket codes (disabled = strip hyphens)
        #[arg(long)]
        keep_hyphens: bool,
        /// Preserve original casing (disabled = uppercase all codes)
        #[arg(long)]
        preserve_case: bool,
    },
    /// Show a summary of the tickets file
    List {
        /// Tickets JSON file to inspect
        #[arg(long, default_value = "tickets.json")]
        store: PathBuf,
    },
}

#[derive(Subcommand)]
enum FaucetCommand {
    /// Start the Axum-based faucet HTTP server
    Serve(FaucetServeArgs),
}

#[derive(Subcommand)]
enum WalletCommand {
    /// Show the balance of the persistent wallet
    Balance(WalletBalanceArgs),
    /// Prefund the persistent wallet without exporting tokens
    Fund(WalletFundArgs),
}

#[derive(Args, Clone)]
struct WalletBalanceArgs {
    /// Cashu mint URL (should match the faucet mint)
    #[arg(long, default_value = "https://m7.mountainlake.io")]
    mint: String,
    /// Optional wallet directory (defaults to ~/.lakeside)
    #[arg(long)]
    wallet_dir: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct WalletFundArgs {
    /// Cashu mint URL
    #[arg(long, default_value = "https://m7.mountainlake.io")]
    mint: String,
    /// Amount to pull into the persistent wallet (in sats)
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    amount: u64,
    /// Optional wallet directory (defaults to ~/.lakeside)
    #[arg(long)]
    wallet_dir: Option<PathBuf>,
    /// Use Bolt12 invoices instead of Bolt11
    #[arg(long)]
    bolt12: bool,
}

#[derive(Args, Clone)]
struct FaucetServeArgs {
    /// Bind address (host:port)
    #[arg(long, default_value = "0.0.0.0:8080")]
    bind: String,
    /// Tickets JSON file to read/update
    #[arg(long, default_value = "tickets.json")]
    tickets: PathBuf,
    /// Cashu mint URL
    #[arg(long, default_value = "https://m7.mountainlake.io")]
    mint: String,
    /// Optional wallet directory (defaults to ~/.lakeside)
    #[arg(long)]
    wallet_dir: Option<PathBuf>,
    #[command(flatten)]
    amount: AmountArgs,
    /// Number of tokens to generate per claim
    #[arg(long, default_value_t = 4)]
    token_count: u64,
    /// Preserve hyphens (disabled = strip hyphens before lookup)
    #[arg(long)]
    keep_hyphens: bool,
    /// Preserve casing (disabled = uppercase before lookup)
    #[arg(long)]
    preserve_case: bool,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Command::Tickets(cmd)) => handle_tickets_command(cmd),
        Some(Command::Faucet(cmd)) => handle_faucet_command(cmd).await,
        Some(Command::Wallet(cmd)) => handle_wallet_command(cmd).await,
        None => run_mint(&cli.mint).await,
    }
}

async fn run_mint(args: &MintArgs) -> Result<()> {
    const MINT_RESERVE: u64 = 10;

    let amount_strategy = args
        .amount
        .resolve()
        .with_context(|| "invalid amount configuration")?;
    let token_values = compute_token_values(amount_strategy, args.token_count);

    let max_amount = compute_sum_total(&token_values) + MINT_RESERVE;
    let mut remaining_credit = max_amount;
    let mut actual_token_count = 0u64;
    let mut tokenvec: Vec<LakesideToken> = Vec::new();

    let lakeside_wallet_type: LakesideWalletType = if args.persistent_wallet {
        let wallet_dir = default_wallet_dir();
        let seed_path = wallet_dir.join("seed");
        let db_path = wallet_dir.join("wallet.sqlite");
        LakesideWalletType::Persistent { seed_path, db_path }
    } else {
        LakesideWalletType::Transient
    };

    let payment_method = if args.bolt12 {
        KnownMethod::Bolt12
    } else {
        KnownMethod::Bolt11
    };

    let lakeside_wallet = LakesideWallet::new(String::from(&args.mint), lakeside_wallet_type);
    let wallet = wallet::mint_all_sats(lakeside_wallet, max_amount, payment_method).await;

    for token_value in &token_values {
        let token_amount = if *token_value > remaining_credit {
            remaining_credit
        } else {
            *token_value
        };

        let token_string = match send_and_export_token(&wallet, token_amount, None).await {
            Ok(ts) => ts,
            Err(err) => {
                if actual_token_count < args.token_count {
                    println!(
                        "Only created {} instead of {} tokens: {:?}",
                        actual_token_count, args.token_count, err
                    );
                }
                break;
            }
        };

        tokenvec.push(LakesideToken {
            value: token_amount,
            code: token_string,
        });

        remaining_credit = remaining_credit.saturating_sub(token_amount);
        actual_token_count += 1;
    }

    let mut all_token_values = String::from("Token values: ");
    let output_path = next_available_filename(&args.output_filename);
    if output_path != PathBuf::from(&args.output_filename) {
        println!(
            "Output file {} exists, writing to {} instead.",
            args.output_filename,
            output_path.display()
        );
    }

    let file = File::create(&output_path)
        .with_context(|| format!("opening output file {}", output_path.display()))?;
    let mut writer = BufWriter::new(file);
    for token in &tokenvec {
        writeln!(writer, "{}\t{}", token.value, token.code)
            .with_context(|| format!("writing token to {}", output_path.display()))?;
        all_token_values.push_str(&token.value.to_string());
        all_token_values.push(' ');
    }

    println!("Tokens written to {}.", output_path.display());
    println!("{}", all_token_values.trim_end());

    Ok(())
}

fn handle_tickets_command(cmd: &TicketsCommand) -> Result<()> {
    match cmd {
        TicketsCommand::Init { output, force } => {
            init_store(output, *force)?;
            println!("Initialized tickets file at {}", output.display());
            Ok(())
        }
        TicketsCommand::Import {
            csv,
            store,
            code_column,
            metadata_columns,
            delimiter,
            keep_hyphens,
            preserve_case,
        } => {
            let options = tickets::ImportOptions {
                csv_path: csv.clone(),
                store_path: store.clone(),
                code_column: code_column.clone(),
                delimiter: *delimiter,
                uppercase: !preserve_case,
                strip_hyphen: !keep_hyphens,
                metadata_columns: metadata_columns.clone(),
            };

            let report = import_from_csv(options)?;
            println!(
                "Imported from {} → {} (new: {}, updated: {}, skipped: {}, total: {})",
                csv.display(),
                store.display(),
                report.inserted,
                report.updated,
                report.skipped,
                report.total_after
            );
            Ok(())
        }
        TicketsCommand::List { store } => {
            let summary = list_summary(store)?;
            let updated_local = summary.updated_at.with_timezone(&Local);
            println!(
                "Tickets file: {} (updated {})",
                summary.path.display(),
                updated_local.format("%Y-%m-%d %H:%M:%S %Z")
            );
            println!(
                "Total: {} | Unclaimed: {} | Claimed: {} | Reissued: {}",
                summary.total, summary.unclaimed, summary.claimed, summary.reissued
            );
            Ok(())
        }
    }
}

async fn handle_faucet_command(cmd: &FaucetCommand) -> Result<()> {
    match cmd {
        FaucetCommand::Serve(args) => run_faucet_server(args).await,
    }
}

async fn handle_wallet_command(cmd: &WalletCommand) -> Result<()> {
    match cmd {
        WalletCommand::Balance(args) => show_wallet_balance(args).await,
        WalletCommand::Fund(args) => fund_wallet(args).await,
    }
}

async fn show_wallet_balance(args: &WalletBalanceArgs) -> Result<()> {
    let wallet_dir = args.wallet_dir.clone().unwrap_or_else(default_wallet_dir);
    let wallet_type = LakesideWalletType::Persistent {
        seed_path: wallet_dir.join("seed"),
        db_path: wallet_dir.join("wallet.sqlite"),
    };
    let lakeside_wallet = LakesideWallet::new(args.mint.clone(), wallet_type);
    let wallet = open_wallet(lakeside_wallet).await;

    let spendable = wallet.total_balance().await?.to_u64();
    let pending = wallet.total_pending_balance().await?.to_u64();
    let reserved = wallet.total_reserved_balance().await?.to_u64();

    println!("Wallet directory: {}", wallet_dir.display());
    println!("Mint: {}", args.mint);
    println!("Spendable balance: {} sats", spendable);
    println!("Pending (awaiting mint): {} sats", pending);
    println!("Reserved (locked for sends): {} sats", reserved);

    Ok(())
}

async fn fund_wallet(args: &WalletFundArgs) -> Result<()> {
    let wallet_dir = args.wallet_dir.clone().unwrap_or_else(default_wallet_dir);
    let wallet_type = LakesideWalletType::Persistent {
        seed_path: wallet_dir.join("seed"),
        db_path: wallet_dir.join("wallet.sqlite"),
    };
    let lakeside_wallet = LakesideWallet::new(args.mint.clone(), wallet_type);
    let method = if args.bolt12 {
        KnownMethod::Bolt12
    } else {
        KnownMethod::Bolt11
    };

    wallet::mint_all_sats(lakeside_wallet, args.amount, method).await;
    println!(
        "Funded persistent wallet with {} sats from {}",
        args.amount, args.mint
    );
    println!("Wallet directory: {}", wallet_dir.display());
    Ok(())
}

async fn run_faucet_server(args: &FaucetServeArgs) -> Result<()> {
    if !args.tickets.exists() {
        bail!(
            "Tickets file {} does not exist (import tickets before starting the faucet)",
            args.tickets.display()
        );
    }

    let addr: SocketAddr = args
        .bind
        .parse()
        .with_context(|| format!("parsing bind address {}", args.bind))?;

    let tickets_path = args.tickets.clone();
    let store = TicketStore::load(&tickets_path)
        .with_context(|| format!("loading tickets file {}", tickets_path.display()))?;

    let wallet_dir = args.wallet_dir.clone().unwrap_or_else(default_wallet_dir);
    let wallet_type = LakesideWalletType::Persistent {
        seed_path: wallet_dir.join("seed"),
        db_path: wallet_dir.join("wallet.sqlite"),
    };
    let lakeside_wallet = LakesideWallet::new(args.mint.clone(), wallet_type);
    let wallet = open_wallet(lakeside_wallet).await;

    let normalization = NormalizationConfig {
        uppercase: !args.preserve_case,
        strip_hyphen: !args.keep_hyphens,
    };
    let amount_strategy = args
        .amount
        .resolve()
        .with_context(|| "invalid amount configuration")?;
    let payout = PayoutConfig {
        strategy: amount_strategy,
        token_count: args.token_count,
    };
    let state = FaucetState::new(tickets_path.clone(), store, wallet, payout, normalization);

    let app = Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/claim", post(claim_ticket))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    println!(
        "Faucet server listening on http://{} (tickets: {})",
        addr,
        tickets_path.display()
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let service = app.into_make_service();
    axum::serve(listener, service)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width,initial-scale=1" />
  <title>Lakeside Faucet</title>
  <style>body{font-family:system-ui,-apple-system,Segoe UI,sans-serif;margin:2rem;line-height:1.5;}code{background:#f4f4f4;padding:0.2em 0.4em;border-radius:4px;}pre{background:#f4f4f4;padding:1rem;border-radius:6px;overflow:auto;}h1{margin-top:0;}a{color:#0b7285;}</style>
</head>
<body>
  <h1>🪣 Lakeside Faucet</h1>
  <p>The backend is running. Use <code>POST /claim</code> to redeem a ticket code or <code>GET /healthz</code> for a JSON health check.</p>
  <h2>Quick test</h2>
  <pre>curl -X POST http://localhost:8080/claim \n  -H 'content-type: application/json' \n  -d '{\"ticket_code\":\"AADJA-62BC3-86259\"}'</pre>
  <p>This minimal page stays in place until the conference UI is wired up.</p>
</body>
</html>"#;

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn claim_ticket(
    State(state): State<FaucetState>,
    Json(request): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, (StatusCode, Json<ErrorResponse>)> {
    if request.ticket_code.trim().is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "invalid_ticket_code",
            "ticket_code must not be empty",
        ));
    }

    let normalized = normalize_ticket_code(
        &request.ticket_code,
        state.shared.normalization.uppercase,
        state.shared.normalization.strip_hyphen,
    );
    if normalized.is_empty() {
        return Err(json_error(
            StatusCode::BAD_REQUEST,
            "invalid_ticket_code",
            "ticket_code is invalid after normalization",
        ));
    }

    let _claim_lock = state.shared.claim_lock.lock().await;

    let mut store_guard = state.shared.tickets.write().await;
    let maybe_ticket = store_guard
        .tickets
        .iter_mut()
        .find(|ticket| ticket.ticket_code == normalized);

    let Some(ticket) = maybe_ticket else {
        return Err(json_error(
            StatusCode::NOT_FOUND,
            "unknown_ticket",
            "Ticket code not found",
        ));
    };

    if !ticket.token_bundles.is_empty() {
        let response = ClaimResponse::from_ticket(ticket, true);
        return Ok(Json(response));
    }

    let values = compute_token_values(
        state.shared.payout.strategy,
        state.shared.payout.token_count,
    );

    // Drop the tickets lock before interacting with the wallet.
    drop(store_guard);

    let mut minted_tokens: Vec<(u64, String)> = Vec::new();
    {
        let wallet_guard = state.shared.wallet.lock().await;
        for amount in values {
            let token_string = send_and_export_token(&wallet_guard, amount, None)
                .await
                .map_err(|err| {
                    json_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "wallet_error",
                        format!("Failed to prepare token: {err}"),
                    )
                })?;
            minted_tokens.push((amount, token_string));
        }
    }

    let mut store_guard = state.shared.tickets.write().await;
    let ticket = store_guard
        .tickets
        .iter_mut()
        .find(|ticket| ticket.ticket_code == normalized)
        .ok_or_else(|| {
            json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ticket_missing",
                "Ticket disappeared while processing claim",
            )
        })?;

    if !ticket.token_bundles.is_empty() {
        let response = ClaimResponse::from_ticket(ticket, true);
        return Ok(Json(response));
    }

    let now = Utc::now();
    ticket.status = TicketStatus::Claimed;
    ticket.claimed_at = Some(now);
    ticket.token_bundles = minted_tokens
        .into_iter()
        .map(|(amount, token)| TokenBundleRecord {
            amount,
            token,
            format: TOKEN_FORMAT_LABEL.to_string(),
            created_at: Some(now),
        })
        .collect();

    let response = ClaimResponse::from_ticket(ticket, false);

    if let Err(err) = store_guard.save(&state.shared.tickets_path) {
        return Err(json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "write_failed",
            format!("Failed to write tickets file: {err}"),
        ));
    }

    Ok(Json(response))
}

fn json_error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            code: code.to_string(),
            message: message.into(),
        }),
    )
}

#[derive(Clone)]
struct FaucetState {
    shared: Arc<FaucetStateInner>,
}

struct FaucetStateInner {
    tickets_path: PathBuf,
    tickets: RwLock<TicketStore>,
    wallet: Mutex<Wallet>,
    payout: PayoutConfig,
    normalization: NormalizationConfig,
    claim_lock: Mutex<()>,
}

impl FaucetState {
    fn new(
        tickets_path: PathBuf,
        store: TicketStore,
        wallet: Wallet,
        payout: PayoutConfig,
        normalization: NormalizationConfig,
    ) -> Self {
        let inner = FaucetStateInner {
            tickets_path,
            tickets: RwLock::new(store),
            wallet: Mutex::new(wallet),
            payout,
            normalization,
            claim_lock: Mutex::new(()),
        };
        FaucetState {
            shared: Arc::new(inner),
        }
    }
}

#[derive(Clone, Copy)]
struct PayoutConfig {
    strategy: AmountStrategy,
    token_count: u64,
}

#[derive(Clone, Copy)]
struct NormalizationConfig {
    uppercase: bool,
    strip_hyphen: bool,
}

#[derive(Deserialize)]
struct ClaimRequest {
    ticket_code: String,
}

#[derive(Serialize)]
struct ClaimResponse {
    status: &'static str,
    already_claimed: bool,
    ticket_code: String,
    display_code: String,
    total_amount: u64,
    tokens: Vec<ClaimToken>,
}

impl ClaimResponse {
    fn from_ticket(ticket: &TicketRecord, already_claimed: bool) -> Self {
        let mut total = 0u64;
        let tokens = ticket
            .token_bundles
            .iter()
            .map(|bundle| {
                total += bundle.amount;
                ClaimToken {
                    amount: bundle.amount,
                    token: bundle.token.clone(),
                    format: bundle.format.clone(),
                    created_at: bundle
                        .created_at
                        .map(|ts| ts.with_timezone(&Utc).to_rfc3339()),
                }
            })
            .collect();

        ClaimResponse {
            status: if already_claimed {
                "already_claimed"
            } else {
                "issued"
            },
            already_claimed,
            ticket_code: ticket.ticket_code.clone(),
            display_code: ticket.display_code.clone(),
            total_amount: total,
            tokens,
        }
    }
}

#[derive(Serialize)]
struct ClaimToken {
    amount: u64,
    token: String,
    format: String,
    created_at: Option<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

struct LakesideToken {
    value: u64,
    code: String,
}

fn default_wallet_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".lakeside")
}

fn next_available_filename(original: &str) -> PathBuf {
    let path = PathBuf::from(original);
    if !path.exists() {
        return path;
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(original);
    let extension = path.extension().and_then(|s| s.to_str());
    let parent = path.parent().map(PathBuf::from);

    for counter in 1u32.. {
        let candidate_name = match extension {
            Some(ext) => format!("{}_{}.{}", stem, counter, ext),
            None => format!("{}_{}", stem, counter),
        };

        let candidate = match &parent {
            Some(dir) => dir.join(&candidate_name),
            None => PathBuf::from(&candidate_name),
        };

        if !candidate.exists() {
            return candidate;
        }
    }

    path
}
