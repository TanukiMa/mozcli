# mozcli

CLI tool to control **Mozc-based Japanese IMEs** (Mozc, Google Japanese Input) on **Windows** and **macOS**.

> Formerly `mozc-cli`.

## Motivation

This tool was born from a conversation with a Mozc developer (yukawa):

> "Mozc CLI ほしいよね。とりあえずは Windows と macOS で mozc_server / mozc_renderer と client を再起動するツールができれば良いと思う。"

## Features

- Restart the IME converter / renderer / client with a single command
- Cross-platform (Windows + macOS)
- Multilingual messages (Japanese / English) via JSON locale files
- Single binary written in Rust

### Roadmap

- `start` / `stop` / `restart` for each supported IME
- Support for Mozc-derived IMEs beyond Google Japanese Input (OSS Mozc builds, etc.)
- `status` command

## Installation

### From source

```bash
git clone https://github.com/TanukiMa/mozcli.git
cd mozcli
cargo build --release
```

The binary will be at `target/release/mozcli` (or `mozcli.exe` on Windows).
Keep the `locales/` directory next to the binary; without it, messages fall back to English.

### Cargo

```bash
cargo install --git https://github.com/TanukiMa/mozcli.git
```

## Usage

```bash
# Restart the IME
mozcli restart

# Force English messages
mozcli --lang en restart

# Force Japanese messages
mozcli --lang ja restart
```

### Language detection

The tool selects the language based on:

1. `--lang` / `-l` option
2. `LC_ALL` / `LC_MESSAGES` / `LANG` environment variables
3. Falls back to English

## Supported IMEs and platforms

Currently only Google Japanese Input process names are handled.

| Platform | Status        | Method                                                   |
|----------|---------------|----------------------------------------------------------|
| Windows  | Supported     | `taskkill /F /T` on known process names                  |
| macOS    | Supported     | `launchctl stop` / `start` on matching launchd services  |
| Linux    | Not supported | —                                                        |

## Contributing

See [AGENTS.md](./AGENTS.md) for guidance intended for contributors and AI coding agents.

## License

MIT
