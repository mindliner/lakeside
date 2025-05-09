use cdk::amount::SplitTarget;
use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::SendOptions;
use cdk::wallet::Wallet;
use cdk::Amount;
use cdk::Error;
use cdk_sqlite::wallet::memory;
use rand::random;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

async fn initialize_wallet(mint: &String) -> Wallet {
    let seed = random::<[u8; 32]>();
    let unit = CurrencyUnit::Sat;
    let localstore = memory::empty().await.unwrap();
    let wallet = Wallet::new(mint, unit, Arc::new(localstore), &seed, None).unwrap();
    wallet
}

pub async fn mint_all_sats(mint: &String, sats_to_mint: u64) -> Wallet {
    let wallet = initialize_wallet(mint).await;
    let amount_to_mint = Amount::from(sats_to_mint);
    let quote = wallet.mint_quote(amount_to_mint, None).await.unwrap();
    println!("Please pay this invoice: {}", quote.request);

    loop {
        let status = wallet.mint_quote_state(&quote.id).await.unwrap();

        if status.state == MintQuoteState::Paid {
            break;
        }
        println!("Quote state: {}", status.state);

        sleep(Duration::from_secs(5)).await;
    }
    println!("Thank you!");

    wallet
        .mint(&quote.id, SplitTarget::default(), None)
        .await
        .unwrap();
    wallet
}

pub async fn mint_and_export_token(wallet: &Wallet, sats: u64) -> Result<String, Error> {
    let prepared_send = wallet
        .prepare_send(Amount::from(sats), SendOptions::default())
        .await?;

    // implement error handling here, return error so that the program can continue
    // to write the tokens to disk
    let token = wallet.send(prepared_send, None).await?;
    Ok(token.to_v3_string())
}
