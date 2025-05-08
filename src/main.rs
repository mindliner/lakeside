//! Wallet example with memory store
//! Note: This example requires the "wallet" feature to be enabled (enabled by default)

use cdk::amount::SplitTarget;
use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::SendOptions;
use cdk::wallet::Wallet;
use cdk::Amount;
use cdk_sqlite::wallet::memory;
use clap::Parser;
use rand::random;
use rand::Rng;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser)]
#[command(about = "A tool to mint and store Cashu tokens of variable amounts", long_about = None)]
#[command(
    name = "lakeside",
    author = "Marius <marius@mountainlake.io>",
    version = "0.1.0",
    about = "Mints Cashu tokens and saves to file"
)]
struct Args {
    /// URL of the Cashu mint
    #[arg(short, long, default_value = "https://mint.mountainlake.io")]
    mint: String,

    /// The value of the token to be issued; use 0 (zero) for tokens of variable amounts and specify the lower and upper bounds
    #[arg(short = 'f', long, required = true)]
    fixed_amount: u64,

    /// In the case of variable token values (fixed_amount is zero), this is the lower bound
    #[arg(short = 'l', long, default_value_t = 10)]
    range_lower_bound: u64,

    /// In the case of variable token values (fixed_amount is zero), this is the upper bound
    #[arg(short = 'u', long, default_value_t = 100)]
    range_upper_bound: u64,

    /// Number of tokens to mint
    #[arg(short = 'n', long, required = true)]
    token_count: u64,

    /// File name to store the amounts and token in a tab separated text file
    #[arg(short, long, default_value = "cashu_tokens.txt")]
    output_filename: String,
}

struct CashuToken {
    value: u64,
    code: String,
}

#[tokio::main]
async fn main() {
    {
        let args = Args::parse();

        // define program parameters
        let seed = random::<[u8; 32]>();
        let unit = CurrencyUnit::Sat;

        //
        // setup wallet and ask to pay lightning invoice
        //
        let max_amount: u64;
        if args.fixed_amount > 0 {
            max_amount = args.fixed_amount * args.token_count;
        } else {
            max_amount = (args.range_lower_bound
                + (args.range_upper_bound - args.range_lower_bound) / 2)
                * args.token_count;
        }

        let amount_minted = Amount::from(max_amount);

        let localstore = memory::empty().await.unwrap();

        let wallet = Wallet::new(&args.mint, unit, Arc::new(localstore), &seed, None).unwrap();

        let quote = wallet.mint_quote(amount_minted, None).await.unwrap();

        println!("Please pay this invoice: {}", quote.request);

        loop {
            let status = wallet.mint_quote_state(&quote.id).await.unwrap();

            if status.state == MintQuoteState::Paid {
                break;
            }
            println!("Quote state: {}", status.state);

            sleep(Duration::from_secs(5)).await;
        }

        let _ = wallet
            .mint(&quote.id, SplitTarget::default(), None)
            .await
            .unwrap();

        let mut remaining_value = max_amount;
        let mut rng = rand::rng();

        let mut tokenvec: Vec<CashuToken> = Vec::new();

        for _ in 0..args.token_count {
            print!(".");
            let mut token_amount: u64;
            if args.fixed_amount == 0 {
                token_amount = rng.random_range(args.range_lower_bound..=args.range_upper_bound);
            } else {
                token_amount = args.fixed_amount;
            }
            if token_amount > remaining_value {
                token_amount = remaining_value;
            }
            // Send the token
            let prepared_send = wallet
                .prepare_send(Amount::from(token_amount), SendOptions::default())
                .await
                .unwrap();

            let token = wallet.send(prepared_send, None).await.unwrap();

            let cashu_token = CashuToken {
                value: token_amount,
                code: token.to_string(),
            };
            tokenvec.push(cashu_token);

            remaining_value = remaining_value - token_amount;
            if remaining_value == 0 {
                break;
            }
        }
        println!("");

        // Open file for writing
        let file = File::create(args.output_filename.clone()).expect("opening file");
        let mut writer = BufWriter::new(file);
        // Write each token line-by-line: value<TAB>code
        for token in &tokenvec {
            writeln!(writer, "{}\t{}", token.value, token.code).expect("Writing token");
        }

        println!("Tokens written to {}", args.output_filename);
    }
}
