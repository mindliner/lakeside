# Changelog

## [0.1.2] - 2026-03-12

### Added
- `--token-format` flag to choose between legacy `cashuA` output and the new `cashuB` (Token V4) envelope.
- Optional `--persistent-wallet` storage in `~/.lakeside/` so minted proofs and seeds survive restarts.
- `--bolt12` switch to request Bolt12 invoices from supporting mints.
- Automatic filename suffixing (`cashu_tokens_1.txt`, `cashu_tokens_2.txt`, …) to avoid truncating existing exports.
- Project README with background on the Bitcoin Lakeside Party raffle origin story.

### Changed
- Default version bumped to `0.1.2` and documentation updated accordingly.
