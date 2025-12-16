///
/// Creates a batch of Cashu tokens of fixed or variable size and outputs
/// these into a text file with each line representing the token's
/// value followed by a space followed by the token's code.
///
/// # Example
///
/// In this example we call the specified mint to create 10 tokens of variable
/// amounts that range between 10 and 100 satoshis. The tokens are store
/// in the default file cashu_tokens.txt.
///
/// ```
/// lakeside -m https://mint.mountainlake.io -f 0 -l 10 -u 100 -n 10
/// ```
///
use clap::Parser;
use std::fs::File;
use std::io::{BufWriter, Write};
use token_amount::{compute_sum_total, compute_token_values};
use wallet::{send_and_export_token, LakesideWallet, LakesideWalletType};

mod token_amount;
mod wallet;

#[derive(Parser)]
#[command(about = "A tool to mint and store Cashu tokens of variable amounts", long_about = None)]
#[command(
    name = "lakeside",
    author = "Marius <marius@mountainlake.io>",
    version = "0.1.1",
    about = "Mints Cashu tokens and exports to file"
)]
struct Args {
    /// URL of the Cashu mint
    //#[arg(short, long, default_value = "https://mint.mountainlake.io")]
    #[arg(short, long, default_value = "https://mint.mountainlake.io")]
    mint: String,

    /// The value of the token to be issued; use 0 for tokens of variable amounts and specify the lower and upper bounds
    #[arg(short = 'f', long, required = true)]
    fixed_amount: u64,

    /// In the case of variable token values (fixed_amount is zero), this is the lower bound
    #[arg(short = 'l', long, default_value_t = 10)]
    lower_bound: u64,

    /// In the case of variable token values (fixed_amount is zero), this is the upper bound
    #[arg(short = 'u', long, default_value_t = 20)]
    upper_bound: u64,

    /// Number of tokens to mint
    #[arg(short = 'n', long, default_value_t = 4)]
    token_count: u64,

    /// File name to store the amounts and token in a tab separated text file
    #[arg(short, long, default_value = "cashu_tokens.txt")]
    output_filename: String,

    /// Persistent wallet; if true the wallet will be stored and re-used, otherwise the wallet will be destroyed at program end
    #[arg(short, long)]
    persistent_wallet: bool,
}

struct LakesideToken {
    value: u64,
    code: String,
}

#[tokio::main]
async fn main() {
    // Todo: check if there is a better way to determine the reserve for the mint
    const MINT_RESERVE: u64 = 10;
    let args = Args::parse();

    let token_values = compute_token_values(
        args.fixed_amount,
        args.lower_bound,
        args.upper_bound,
        args.token_count,
    );

    let max_amount = compute_sum_total(&token_values) + MINT_RESERVE;
    let mut remaining_credit = max_amount;
    let mut actual_token_count = 0;
    let mut tokenvec: Vec<LakesideToken> = Vec::new();
    let lakeside_wallet_type: LakesideWalletType = if args.persistent_wallet {
        LakesideWalletType::Persistent(String::from("/home/marius/.lakeside/seed"))
    } else {
        LakesideWalletType::Transient
    };

    let lakeside_wallet = LakesideWallet::new(String::from(&args.mint), lakeside_wallet_type);

    let wallet = wallet::mint_all_sats(lakeside_wallet, max_amount).await;

    for t in &token_values {
        let token_amount: u64 = if *t > remaining_credit {
            remaining_credit
        } else {
            *t
        };

        let token_string: String = match send_and_export_token(&wallet, token_amount).await {
            Ok(ts) => ts,
            Err(cdkerr) => {
                if actual_token_count < args.token_count {
                    println!(
                        "Only created {} instead of {} tokens: {:?}",
                        actual_token_count, args.token_count, cdkerr
                    );
                }
                break;
            }
        };

        let cashu_token = LakesideToken {
            value: token_amount,
            code: token_string,
        };
        tokenvec.push(cashu_token);

        remaining_credit = remaining_credit - token_amount;
        actual_token_count += 1;
    }

    let mut all_token_values = String::from("Token values: ");
    // Open file for writing
    let file = File::create(args.output_filename.clone()).expect("opening file");
    let mut writer = BufWriter::new(file);
    // Write each token line-by-line: value<TAB>code
    for token in &tokenvec {
        writeln!(writer, "{}\t{}", token.value, token.code).expect("Writing token");
        all_token_values.push_str(&token.value.to_string());
        all_token_values.push(' ');
    }

    println!("Tokens written to {}.", args.output_filename);
    println!("{}", all_token_values);
}
