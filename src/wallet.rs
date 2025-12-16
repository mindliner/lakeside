use std::sync::Arc;
use std::time::Duration;
use cdk::amount::SplitTarget;
use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::SendOptions;
use cdk::wallet::Wallet;

use cdk::Amount;
use cdk::Error;
use cdk_sqlite::wallet::memory;
use core::panic;
use rand::random;
use std::{fs, io};
use tokio::time::sleep;

pub enum LakesideWalletType {
    Transient,
    Persistent(String),
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

async fn initialize_wallet(lakeside_mint: LakesideWallet) -> Wallet {
    //let seed = random::<[u8; 32]>();
    let seed = load_or_generate_seed(&lakeside_mint).unwrap();
    let mint_url = "https://fake.thesimplekid.dev";

    let unit = CurrencyUnit::Sat;
    let wallet: Wallet;
    match lakeside_mint.wallet_type {
        LakesideWalletType::Transient => {
            let localstore = memory::empty().await.unwrap();
            wallet = Wallet::new(&lakeside_mint.mint, unit, Arc::new(localstore), seed, None).unwrap();
        }
        LakesideWalletType::Persistent(wallet_mint) => {
            panic!("Persistent wallet not yet implemented.")
        }
    }
    wallet
}

pub async fn mint_all_sats(lakeside_wallet: LakesideWallet, sats_to_mint: u64) -> Wallet {
    let wallet = initialize_wallet(lakeside_wallet).await;
    let amount_to_mint = Amount::from(sats_to_mint);
    let quote = wallet.mint_quote(amount_to_mint, None).await.unwrap();

    println!("Please pay this invoice: {}", quote.request);
    loop {
        let status = wallet.mint_quote_state(&quote.id).await.unwrap();

        if status.state == MintQuoteState::Paid {
            break;
        }
        println!("...waiting for payment, state: {}", status.state);

        sleep(Duration::from_secs(5)).await;
    }
    println!("Received!");

    wallet
        .mint(&quote.id, SplitTarget::default(), None)
        .await
        .unwrap();
    wallet
}

pub async fn send_and_export_token(wallet: &Wallet, sats: u64) -> Result<String, Error> {
    let prepared_send = wallet
        .prepare_send(Amount::from(sats), SendOptions::default())
        .await?;

    // implement error handling here, return error so that the program can continue
    // to write the tokens to disk
    let token = prepared_send.confirm(None).await?;
    Ok(token.to_v3_string())
}

fn load_or_generate_seed(mint: &LakesideWallet) -> io::Result<[u8; 64]> {
    let seed: [u8; 64];
    match mint.wallet_type {
        LakesideWalletType::Transient => {
            seed = random::<[u8; 64]>();
        }
        LakesideWalletType::Persistent(ref url) => match fs::read(url) {
            Ok(s) => {
                seed = s[..].try_into().unwrap();
                println!("Loaded existing seed from file");
            }
            Err(e) => {
                println!("Could not load seed, generating new one: {:?}", e);
                seed = random::<[u8; 64]>();
                fs::write(url, &seed)?;
            }
        },
    }
    Ok(seed)
}
