# mozcli

CLI tool to control **Mozc-based Japanese IMEs** (Mozc, Google Japanese Input) on **Windows** and **macOS**.

> Formerly `mozc-cli`.

## Motivation

This tool was born from a conversation with a Mozc developer (yukawa):

> "Mozc CLI ほしいよね。とりあえずは Windows と macOS で mozc_server / mozc_renderer と client を再起動するツールができれば良いと思う。"

## Features

- `start` / `stop` / `restart` the IME with a single command
- Cross-platform (Windows + macOS)
- Multilingual messages (Japanese / English), embedded in the binary
- Single binary written in Rust

### Roadmap

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

### Cargo

```bash
cargo install --git https://github.com/TanukiMa/mozcli.git
```

## Usage

```bash
mozcli start            # start the IME server (Windows: see note below)
mozcli stop             # stop the IME server and renderer
mozcli restart          # stop, then start

mozcli --ime google restart   # target one IME (default: every installed IME)
mozcli --lang en restart      # force English messages (or --lang ja)
```

Exit code is `0` on success and `1` if no IME was found or a command failed.

On Windows, the IME server cannot be started directly. After `stop` or `restart`, it starts again automatically the next time you type, so `start` only checks that the IME is installed.

### Language detection

The tool selects the language based on:

1. `--lang` / `-l` option
2. `LC_ALL` / `LC_MESSAGES` / `LANG` environment variables
3. Falls back to English

## Supported IMEs and platforms

| `--ime`  | IME                   |
|----------|-----------------------|
| `google` | Google Japanese Input |

| Platform | Status        | stop                         | start                               |
|----------|---------------|------------------------------|-------------------------------------|
| Windows  | Supported     | `taskkill /F /T` per process | automatic on next input             |
| macOS    | Supported     | `launchctl stop <label>`     | `launchctl start <label>`           |
| Linux    | Not supported | —                            | —                                   |

## Contributing

See [AGENTS.md](./AGENTS.md) for guidance intended for contributors and AI coding agents.

## License

MIT
