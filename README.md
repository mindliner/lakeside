# Lakeside

Lakeside is a small Rust CLI that mints Cashu ecash tokens in batches and writes
them out as tab-separated text files. It started as the tooling behind the
**Bitcoin Lakeside Party 2025 – Grill & Chill**, where every guest could scan the
QR on their ticket and draw three randomly sized Cashu gifts. The name stuck.

## Highlights

- 🪙 **Fixed or variable denominations** – mint a pile of identical tokens or
  random values inside a range.
- ⚡ **Bolt11 _or_ Bolt12 invoices** – talk to modern mints without leaving
  older ones behind.
- 📦 **cashuA / cashuB output** – choose the legacy-friendly Token V3 format or
  the newer Token V4 envelope.
- 💾 **Persistent wallet state (optional)** – keep the wallet database and seed
  in `~/.lakeside/` so minted proofs survive crashes and can be re-exported.
- 📝 **Plain-text export** – each line is `amount<TAB>cashu…`, perfect for
  sticking into QR codes, tickets, or promo cards.

## Installation

```bash
# inside this repo
cargo build --release
```

Rust 1.94+ is required. If `cargo` is missing, install it via [rustup](https://rustup.rs).

## Usage

```
lakeside \
  --mint https://mint.mountainlake.io \
  --fixed-amount 0 \
  --lower-bound 10 \
  --upper-bound 100 \
  --token-count 10 \
  --output-filename cashu_tokens.txt \
  --token-format cashuA \
  --persistent-wallet
```

Key flags:

| Flag | Description |
| --- | --- |
| `-m, --mint` | Cashu mint URL. |
| `-f, --fixed-amount` | Set to `0` for variable amounts (use `--lower-bound` / `--upper-bound`). |
| `-n, --token-count` | Number of tokens to create. |
| `-o, --output-filename` | Base filename; Lakeside auto-appends `_1`, `_2`, … if the file exists. |
| `--token-format` | `cashuA` (default) or `cashuB`. |
| `--bolt12` | Use Bolt12 invoices instead of Bolt11. |
| `-p, --persistent-wallet` | Store seed + wallet DB in `~/.lakeside/` so proofs persist. |

Remember to insert `--` when running via Cargo:

```bash
cargo run -- --mint https://mint.mountainlake.io --fixed-amount 21 --token-count 5
```

## Outputs

Each successful run creates a file similar to:

```
Token values: 12 38 79 24 17 
12    cashuAeyJ0b2tlbiI6IFt7Im1pbnQiOiAiaHR0cHM6Ly9taW50LiIsICJwcm9vZnMiOiBb...
38    cashuAeyJ0b2tlbiI6IFt7Im1pbnQiOiAiaHR0cHM6Ly9taW50LiIsICJwcm9vZnMiOiBb...
```

If the requested file already exists, Lakeside picks the next free name (e.g.
`cashu_tokens_1.txt`).

## Why "Lakeside"?

In May 2025 we hosted the **Bitcoin Lakeside Party – Grill & Chill**. Guests
received QR-coded tickets; scanning the code called this tool, which minted
three surprise Cashu gifts per person. Lakeside was the internal codename for
that raffle app, and it became the project name as the tool grew up.

## License

MIT
