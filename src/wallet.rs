use cdk::amount::SplitTarget;
use cdk::nuts::nut00::KnownMethod;
use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::{SendOptions, Wallet};
use cdk::Amount;
use cdk::Error;
use cdk_sqlite::wallet::{memory, WalletSqliteDatabase};
use rand::random;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, io};
use tokio::time::sleep;

pub enum LakesideWalletType {
    Transient,
    Persistent {
        seed_path: PathBuf,
        db_path: PathBuf,
    },
}

pub struct LakesideWallet {
    mint: String,
    wallet_type: LakesideWalletType,
}

impl LakesideWallet {
    pub fn new(mint: String, wallet_type: LakesideWalletType) -> Self {
        Self { mint, wallet_type }
    }
}

pub async fn open_wallet(lakeside_wallet: LakesideWallet) -> Wallet {
    initialize_wallet(lakeside_wallet).await
}

async fn initialize_wallet(lakeside_mint: LakesideWallet) -> Wallet {
    let LakesideWallet { mint, wallet_type } = lakeside_mint;
    let seed = load_or_generate_seed(&wallet_type).expect("load seed");

    let unit = CurrencyUnit::Sat;
    let localstore = match wallet_type {
        LakesideWalletType::Transient => memory::empty().await.expect("wallet store"),
        LakesideWalletType::Persistent { db_path, .. } => {
            ensure_parent_dir(&db_path).expect("wallet dir");
            WalletSqliteDatabase::new(db_path)
                .await
                .expect("open wallet database")
        }
    };

    Wallet::new(&mint, unit, Arc::new(localstore), seed, None).expect("create wallet")
}

pub async fn mint_all_sats(
    lakeside_wallet: LakesideWallet,
    sats_to_mint: u64,
    payment_method: KnownMethod,
) -> Wallet {
    let wallet = initialize_wallet(lakeside_wallet).await;
    let amount_to_mint = Amount::from(sats_to_mint);
    let quote = wallet
        .mint_quote(payment_method, Some(amount_to_mint), None, None)
        .await
        .unwrap();

    println!("Please pay this invoice: {}", quote.request);
    loop {
        let quote_status = wallet.check_mint_quote_status(&quote.id).await.unwrap();

        match quote_status.state {
            MintQuoteState::Paid | MintQuoteState::Issued => break,
            other => {
                println!("...waiting for payment, state: {:?}", other);
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
    println!("Received!");

    wallet
        .mint(&quote.id, SplitTarget::default(), None)
        .await
        .unwrap();
    wallet
}

pub async fn send_and_export_token(
    wallet: &Wallet,
    sats: u64,
    options: Option<SendOptions>,
) -> Result<String, Error> {
    let send_options = options.unwrap_or_else(SendOptions::default);
    let prepared_send = wallet
        .prepare_send(Amount::from(sats), send_options)
        .await?;

    // implement error handling here, return error so that the program can continue
    // to write the tokens to disk
    let token = prepared_send.confirm(None).await?;
    let token_string = token.to_string();
    Ok(token_string)
}

fn load_or_generate_seed(wallet_type: &LakesideWalletType) -> io::Result<[u8; 64]> {
    match wallet_type {
        LakesideWalletType::Transient => Ok(random::<[u8; 64]>()),
        LakesideWalletType::Persistent { seed_path, .. } => match fs::read(seed_path) {
            Ok(bytes) if bytes.len() == 64 => {
                let mut seed = [0u8; 64];
                seed.copy_from_slice(&bytes);
                println!("Loaded existing seed from {}", seed_path.display());
                Ok(seed)
            }
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Seed file must contain exactly 64 bytes",
            )),
            Err(_) => {
                ensure_parent_dir(seed_path)?;
                let seed = random::<[u8; 64]>();
                fs::write(seed_path, &seed)?;
                Ok(seed)
            }
        },
    }
}

fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}
