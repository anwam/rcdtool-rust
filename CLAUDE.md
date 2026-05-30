# rcdtool-rust

Rust rewrite of `rcdtool` — a CLI that downloads media from Telegram messages
(including restricted/private channels you are a member of) via MTProto.

The reference Python implementation lives at `../../src/rcdtool/` (Telethon-based).
This crate ports it to Rust on top of **grammers** (pure-Rust MTProto client).

## Build / test / run

```sh
cd rust/rcdtool-rust
cargo build
cargo test          # 5 parser unit tests in src/utils.rs
cargo run -- --help

# dry run (no network, no auth): prints the planned output filenames
cargo run -- -c ../../config.ini --link "https://t.me/c/1234567890/851" --dry-run
```

`config.ini` lives at the repo root (`../../config.ini`); pass it with `-c`.

## Module layout

- `main.rs` — `#[tokio::main]` entrypoint. Parses CLI, loads config, builds the
  list of `(channel, message)` targets (`collect_targets`), allocates unique
  output filenames, then runs all downloads concurrently via `join_all`.
- `cli.rs` — clap `Arguments`. `parse_compat()` rewrites the legacy `-DM` flag
  to `--discussion-message-id` before parsing (clap reserves single-dash longs).
- `config.rs` — INI loader for `[Access]` (session, id, hash) and `[Client]`
  (timeout, device_model, lang_code). `[Client]` fields are parsed for
  compat but **not applied** — grammers 0.9 no longer exposes them via its
  high-level client API (hence `#[allow(dead_code)]`).
- `utils.rs` — pure parsing logic ported from `utils.py`:
  `parse_channel_id` (numeric → `-100…` marked id, or `@username`),
  `parse_message_id`, `parse_ranges` (`"1638,1639..1641"`),
  `parse_targets_from_link` (handles `t.me/c/<id>/<topic>/<msg>` and plain links),
  `generate_unique_filename` (adds `-1`, `-2`, … on collision; supports
  `--detailed-name` and an exclude list for in-batch collisions).
- `telegram.rs` — all grammers I/O: `connect` (SenderPool + spawn runner +
  interactive login w/ 2FA), `resolve_peer` (username → resolve, numeric →
  scan dialogs by `bot_api_dialog_id`), `download`, `apply_inferred_extension`.
- `downloader.rs` — `Downloader` orchestrates per-target: resolve peer, fetch
  message, download media, optionally infer+rename extension. In `--dry-run`
  it skips all network access and just echoes the output filename.

## grammers 0.9 API notes (these bit me; pin to 0.9 semantics)

grammers 0.9 is a **major rework** vs older releases. Key facts:

- Deps: `grammers-client = { version = "0.9", features = ["fs"] }` (the `fs`
  feature gates `Client::download_media`), `grammers-session = { version = "0.9",
  features = ["sqlite-storage"] }` (feature is `sqlite-storage`, **not** `sqlite`).
- Connect pattern (no more `Client::connect`/`Config`):
  ```rust
  let session = Arc::new(SqliteSession::open(path).await?);
  let SenderPool { runner, handle, .. } = SenderPool::new(Arc::clone(&session), api_id);
  let client = Client::new(handle);
  tokio::spawn(runner.run());   // drives the connection; stops when handles drop
  ```
- `request_login_code(phone, api_hash)` takes **two** args (api_hash at login,
  not at construction). `sign_in` returns `SignInError::PasswordRequired(token)`
  for 2FA → `check_password(token, pwd)`. `user.first_name()` is `Option<&str>`.
- `resolve_username` → `Option<Peer>`. To use a peer in requests you need a
  `PeerRef` via `peer.to_ref().await` (`Option`, only Some if cached/usable).
- Numeric private channels: grammers can only address chats already in your
  session cache, so we scan `client.iter_dialogs()` and match
  `dialog.peer().id().bot_api_dialog_id()` against the `-100…` marked id.
- `client.get_messages_by_id(peer, &[id])` → `Vec<Option<Message>>`.
  `message.media()` → `Option<Media>`; `client.download_media(&media, path)`.
- Session is its own SQLite format (incompatible with Telethon). We use a
  separate `<session-name>.grammers.session` file so we never clobber the
  Python tool's session.

## Status (as of 2026-05-29)

**Working & build/test-clean** (`cargo build`/`test` pass, no warnings):
- CLI parity incl. legacy `-DM`.
- INI config loading.
- All parsing logic + 5 unit tests.
- `--dry-run` end-to-end (verified: single link, range, `-C/-M` comma+range,
  `-O` custom path, `--detailed-name`).
- **Real Grammers integration**: connect, interactive login (phone/code/2FA),
  session persistence, username + numeric-channel resolution, message fetch,
  media download, `--infer-extension` rename.
- **Discussion message download (`-DM`)**: calls `GetDiscussionMessage` raw TL
  to find the linked discussion group, then fetches the specific message from
  that group and downloads its media.

**Not yet ported / TODO:**
- Paid media (`MessageMediaPaidMedia` / `extended_media`) — Python handles it;
  grammers 0.9 `Media::from_raw` returns `None` for `PaidMedia`, so it's skipped.
- No live end-to-end test against real Telegram yet (needs real API creds +
  interactive login). Only dry-run is exercised here.

## Gotcha for future sessions

The crate is at `rust/rcdtool-rust/` (NOT `rust/`). `cd` does not persist
between Bash tool calls in this harness — use
`cargo --manifest-path rust/rcdtool-rust/Cargo.toml …` or absolute paths.
