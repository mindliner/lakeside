# Changelog

## [Unreleased]
- _Nothing yet._

## [0.1.3] - 2026-03-13

### Added
- Ticket datastore tooling (`tickets init/import/list`) plus hashed ticket storage.
- Axum-based faucet server (`faucet serve`) with `/`, `/healthz`, and `/claim` endpoints that bind one Cashu bundle to each ticket code.
- Wallet maintenance commands: `wallet fund` (preload sats without exporting) and `wallet balance` (spendable/pending/reserved snapshot).
- Static status page and JSON responses that replay previously issued bundles on re-requests.

### Changed
- cashu-b is now the only export format (CLI flag removed) and all stored bundles include `format: "cashu-b"`.
- Amount flags are mutually exclusive: provide either `--fixed-amount` or both `--lower-bound`/`--upper-bound` (with validation shared between CLI and faucet).
- README updated to document the new workflow and endpoints.

## [0.1.2] - 2026-03-12

### Added
- `--token-format` flag to choose between legacy `cashuA` output and the new `cashuB` (Token V4) envelope.
- Optional `--persistent-wallet` storage in `~/.lakeside/` so minted proofs and seeds survive restarts.
- `--bolt12` switch to request Bolt12 invoices from supporting mints.
- Automatic filename suffixing (`cashu_tokens_1.txt`, `cashu_tokens_2.txt`, …) to avoid truncating existing exports.
- Project README with background on the Bitcoin Lakeside Party raffle origin story.

### Changed
- Default version bumped to `0.1.2` and documentation updated accordingly.
