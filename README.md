# rcdtool-rust

`rcdtool-rust` is a Rust CLI for downloading Telegram media from private or restricted channels via MTProto.

> **Based on [rcdtool](https://github.com/David256/rcdtool) by [@David256](https://github.com/David256).**
> This project is a full port of his Python/Telethon implementation to Rust using the [grammers](https://github.com/Lonami/grammers) MTProto client.

### Additions over the original

- **Batch link files** — pass a `.txt` file of links via `--link-file` (one URL per line, `#` comments supported)
- **Discussion message ranges** — `?comment=101..105` in a link expands into individual download targets for each comment ID
- **Structured output layout** — files are saved under `download/{channel}/{batch_id}/{message-id}.{ext}` (no-DM) or `download/{channel}/{message-id}/{dm-id}.{ext}` (with discussion ID) automatically, no `-O` flag needed
- **Hash batch folder** — when downloading a range without a discussion ID, all files in the same batch share a stable 8-hex-char folder name derived from the link, preventing cross-batch collisions
- **Extension inference on by default** — `--infer-extension` is enabled by default; downloaded files are automatically renamed with the correct extension

## Prerequisites

- Rust toolchain (install from [rustup.rs](https://rustup.rs/))
- telegram app_id and app_hash [Create telegram app](https://my.telegram.org/)

## Build

```sh
cargo build
cargo test
```

## Run

- Copy `config.ini.sample` to `config.ini` and fill in your Telegram credentials and session data.
- Run the tool with message links or .txt files containing links. For example:
```sh
cargo run -- -c config.ini --link "https://t.me/c/1234567890/851"
```

or with multiple links:
```sh
cargo run -- -c config.ini --link "https://t.me/c/1234567890/851;https://t.me/c/1234567890/852"
```

or with link file:
```sh
cargo run -- -c config.ini --link-file "links.txt"
```

### the `links.txt` file should contain one message link per line, for example:
```txt
https://t.me/c/1234567890/851
```

or ranges of comment IDs:
```txt
https://t.me/c/1234567890/851?comment=101..105
```

Useful flags:

- `-c, --config <FILE>`: config file path, default `config.ini`
- `--link <VALUES>`: one or more message links and/or `.txt` files (one link per line), separated by `;`
- `--link-file <FILES>`: one or more `.txt` files containing links (one link per line), separated by `;` or repeated
- `-C, --channel-id <ID>`: channel ID or username
- `-M, --message-id <IDS>`: message IDs and ranges, for example `10,11..15`
- `-D, --discussion-message-id <ID>`: discussion message ID for linked groups
- `-O, --output <FILE>`: output filename
- `--infer-extension`: infer and rename the downloaded file extension (enabled by default)
- `--detailed-name`: include channel and message IDs in the filename
- `--dry-run`: print planned filenames without downloading

Link examples:

- `https://t.me/JustinWDUM2/25?comment=101`
- `https://t.me/JustinWDUM2/25?comment=101..105`

Default output layout (when `-O/--output` is not set):

- `download/{channel}/{message-id}/{message-id}`
- `download/{channel}/{message-id}/{discussion-message-id}` when discussion message is used
- For entries loaded from `--link-file` with no discussion message ID: `download/{channel}/{message-id}`

By default, the inferred extension is appended to the output file.

## Config

The default `config.ini` should provide Telegram access credentials and session data. See the bundled `config.ini` for the expected format.
