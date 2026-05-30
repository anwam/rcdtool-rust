# rcdtool-rust

Rust port of https://github.com/David256/rcdtool.
Downloads Telegram media via MTProto (grammers), including restricted/private channels your account can access.

## Quick commands

```sh
cargo build
cargo test
cargo run -- --help

# dry run (no auth/network download side-effects)
cargo run -- -c config.ini --link "https://t.me/c/1234567890/851" --dry-run
```

## Current status (2026-05-30)

Implemented:
- Link parsing for standard links, `t.me/c/...`, and message ranges.
- Discussion message support with `-D/--discussion-message-id`.
- Discussion comment parsing from URL query (`?comment=101` and `?comment=101..105`).
- `--link-file` input (`.txt`, one link per line, `#` comment lines ignored).
- Legacy `.txt` ingestion through `--link` (semicolon-separated values still supported).
- Default extension inference enabled (`--infer-extension` defaults to true).
- Default output layout:
  - No discussion ID: `download/{channel}/{batch_id}/{message-id}.{ext}`
  - With discussion ID: `download/{channel}/{message-id}/{discussion-message-id}.{ext}`
- Stable `batch_id` folder (8-hex hash derived from link/input batch key) to avoid collisions.
- Concurrent downloads capped at 2 by default via `--concurrency`.
- Dry-run mode prints planned output paths.

Not yet implemented:
- Paid media handling (`MessageMediaPaidMedia` / extended media path in original Python tool).
- Automated live integration test against Telegram (requires real credentials + interactive auth).

## File map

- `src/main.rs`: CLI flow, target expansion, batch hash generation, and request orchestration.
- `src/cli.rs`: clap arguments and legacy `-DM` compatibility rewrite.
- `src/utils.rs`: parsing helpers and output path helper.
- `src/telegram.rs`: grammers connection/auth/peer resolution/download internals.
- `src/downloader.rs`: per-target execution wrapper and local output directory prep.
- `src/config.rs`: `config.ini` loader.

## Important notes

- `config.ini` is required (`-c config.ini` by default).
- Grammers session is stored as `<session>.grammers.session` and is separate from Telethon sessions.
- Numeric private channels can only be resolved if they exist in current account dialogs cache.
