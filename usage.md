# Lakeside Usage Guide

This file walks through the conference faucet workflow: load tickets, fund the wallet, serve claims, and verify the response. Run `lakeside --help` or `lakeside <subcommand> --help` for additional flags.

---

## 1. Import ticket data

```bash
# initialize the datastore (once)
lakeside tickets init --output tickets.json

# convert CSV export of your ticketing system into the datastore
lakeside tickets import \
  --csv attendees.csv \
  --code-column ticket_code \
  --metadata-column holder_name \
  --store tickets.json
```

## 2. Fund the persistent wallet

```bash
# pulls 50,000 sats into ~/.lakeside/wallet.sqlite
lakeside wallet fund \
  --amount 50000 \
  --mint https://m7.mountainlake.io

# optional: verify balances
lakeside wallet balance --mint https://m7.mountainlake.io
```

## 3. Start the faucet server

Example 1: Serve one Cashu gift of 1,212 sats per ticket (fixed amount).

```bash
lakeside faucet serve \
  --tickets tickets.json \
  --bind 0.0.0.0:8080 \
  --mint https://m7.mountainlake.io \
  --fixed-amount 1212 \
  --token-count 1
```

Example 2: Serve **three** Cashu gifts per ticket with random values between 1,000 and 4,000 sats.

```bash
lakeside faucet serve \
  --tickets tickets.json \
  --bind 0.0.0.0:8080 \
  --mint https://m7.mountainlake.io \
  --lower-bound 1000 \
  --upper-bound 4000 \
  --token-count 3
```

Common flags:

- `--tickets` – path to the datastore created via `tickets init/import`.
- `--bind` – address/port for the Axum server (defaults to `127.0.0.1:8080`).
- `--mint` – Cashu mint that will issue the proofs.
- `--fixed-amount` *or* (`--lower-bound` + `--upper-bound`) – token values per ticket.
- `--token-count` – number of tokens per ticket.

## 4. Claim a token bundle

Using the server from Example 1:

```bash
curl -X POST http://127.0.0.1:8080/claim \
  -H 'content-type: application/json' \
  -d '{"ticket_code":"AADJA-62BC3-1234"}'
```

Expected response (truncated token for brevity):

```json
{
  "status": "issued",
  "already_claimed": false,
  "ticket_code": "AADJA62BC31234",
  "display_code": "AADJA-62BC3-1234",
  "total_amount": 1212,
  "tokens": [
    {
      "amount": 1212,
      "token": "cashuBo2Ft…",
      "format": "cashu-b",
      "created_at": "2026-03-16T10:25:09.657866295+00:00"
    }
  ]
}
```

- When a ticket is claimed again, `status` becomes `"already_claimed"` and the same bundle is replayed.
- Ticket codes are normalized (uppercase, hyphen stripping) before lookup.

## Troubleshooting tips

- Run `lakeside tickets list --store tickets.json` to inspect claimed vs unclaimed counts.
- `RUST_LOG=lakeside=debug` before `lakeside faucet serve …` adds verbose server logs.
- If funding fails, re-run `lakeside wallet fund` with `--bolt12` if your mint prefers modern invoices.
