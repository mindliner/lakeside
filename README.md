# Lakeside

Lakeside started as a small Rust CLI for batch-minting Cashu tokens for the **Bitcoin Lakeside Party 2025 – Grill & Chill**. It has since grown into a ticket-aware faucet server plus utilities for importing attendee lists, funding a persistent wallet, and handing out bundles of sats at conferences.

## Highlights

- 🎟️ **Ticket-gated faucet** – Axum server with `/claim` that ties one Cashu bundle to each ticket code, stores issued tokens, and replays them if a guest needs them again.
- 🪙 **Fixed or variable denominations** – the legacy CLI still mints piles of identical tokens or random values inside a range.
- ⚡ **Bolt11 _or_ Bolt12 invoices** – talk to modern mints without leaving older ones behind.
- 💾 **Persistent wallet with tooling** – keep proofs in `~/.lakeside/`, top them up via `lakeside wallet fund`, and inspect balances with `lakeside wallet balance`.
- 📄 **Plain-text export (optional)** – each line remains `amount<TAB>cashu…`, handy for QR stickers or promo cards.
- 🧾 **cashu-b everywhere** – the faucet and CLI now emit the modern Token V4 envelope exclusively.

## Installation

```bash
# inside this repo
cargo build --release
```

Rust 1.94+ is required. If `cargo` is missing, install it via [rustup](https://rustup.rs).

## Minting tokens to a file (legacy CLI)

The default command (no subcommand) still mints a batch of tokens and writes them to a tab-separated file:

```
lakeside \
  --mint https://mint.mountainlake.io \
  --lower-bound 10 \
  --upper-bound 100 \
  --token-count 10 \
  --output-filename cashu_tokens.txt \
  --persistent-wallet
```

Key flags:

| Flag | Description |
| --- | --- |
| `-m, --mint` | Cashu mint URL. |
| `-f, --fixed-amount` | Positive amount per token (mutually exclusive with `--lower-bound`/`--upper-bound`). |
| `-n, --token-count` | Number of tokens to create. |
| `-o, --output-filename` | Base filename; Lakeside auto-appends `_1`, `_2`, … if the file exists. |
| `--bolt12` | Use Bolt12 invoices instead of Bolt11. |
| `-p, --persistent-wallet` | Store seed + wallet DB in `~/.lakeside/` so proofs persist and can be re-exported. |

> Tip: provide either `--fixed-amount` *or* both `--lower-bound` and `--upper-bound`. If you omit all three, Lakeside defaults to 10–20 sats per token.

Remember to insert `--` when running via Cargo:

```bash
cargo run -- --mint https://mint.mountainlake.io --fixed-amount 21 --token-count 5
```

## Ticket faucet quick start

1. **Create & import tickets**
   ```bash
   lakeside tickets init --output tickets.json
   lakeside tickets import \
     --csv attendees.csv \
     --code-column ticket_code \
     --metadata-column holder_name \
     --store tickets.json
   ```
2. **Fund the persistent wallet**
   ```bash
   lakeside wallet fund --amount 50000 --mint https://m7.mountainlake.io
   lakeside wallet balance --mint https://m7.mountainlake.io
   ```
3. **Start the faucet server**
   ```bash
   lakeside faucet serve \
     --tickets tickets.json \
     --bind 0.0.0.0:8080 \
     --mint https://m7.mountainlake.io \
        --lower-bound 10 \
     --upper-bound 20 \
     --token-count 4
   ```
4. **Test a claim**
   ```bash
   curl -X POST http://localhost:8080/claim \
     -H 'content-type: application/json' \
     -d '{"ticket_code":"AADJA-62BC3-86259"}'
   ```

Endpoints:

- `GET /` – minimal HTML status page.
- `GET /healthz` – JSON `{"status":"ok"}`.
- `POST /claim` – `{ "ticket_code": "..." }` → returns the token bundle (`status` is `issued` or `already_claimed`).

## Wallet commands

| Command | Description |
| --- | --- |
| `lakeside wallet fund --amount 50000` | Pays a Lightning invoice, mints proofs, and keeps them in `~/.lakeside/` (no export). |
| `lakeside wallet balance` | Shows spendable / pending / reserved sats in the persistent wallet. |

Both support `--mint`, `--wallet-dir`, and `--bolt12` (for funding).

## Ticket commands

| Command | Description |
| --- | --- |
| `lakeside tickets init --output tickets.json` | Bootstraps an empty datastore. |
| `lakeside tickets import --csv attendees.csv --code-column ticket_code` | Normalizes ticket codes (uppercase, hyphen stripping by default), hashes them, and merges metadata. |
| `lakeside tickets list --store tickets.json` | Prints totals (claimed vs unclaimed). |

## Outputs

The legacy CLI still produces files like:

```
Token values: 12 38 79 24 17 
12    cashuBeyJ0b2tlbiI6IFt7Im1pbnQiOiAiaHR0cHM6Ly9taW50LiIsICJwcm9vZnMiOiBb...
38    cashuBeyJ0b2tlbiI6IFt7Im1pbnQiOiAiaHR0cHM6Ly9taW50LiIsICJwcm9vZnMiOiBb...
```

If the requested file already exists, Lakeside picks the next free name (e.g. `cashu_tokens_1.txt`). The faucet stores the same structure in `tickets.json` under each ticket entry.

## Why "Lakeside"?

In May 2025 we hosted the **Bitcoin Lakeside Party – Grill & Chill**. Guests
received QR-coded tickets; scanning the code called this tool, which minted
three surprise Cashu gifts per person. Lakeside was the internal codename for
that raffle app, and it became the project name as the tool grew up.

## License

MIT
