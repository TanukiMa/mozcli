# AGENTS.md — mozcli

Guidance for contributors and AI coding agents. Keep this file in sync with the code.

## Project overview

`mozcli` (formerly `mozc-cli`) is a Rust command-line tool for controlling Mozc-based Japanese IMEs on **Windows** and **macOS**.

It started from a request by a Mozc developer (yukawa):

> "We want a Mozc CLI. For now, a tool that can restart `mozc_server`, `mozc_renderer`, and the client on Windows and macOS would be good."

**Direction:** grow from "restart Google Japanese Input" into `start` / `stop` / `restart` for Mozc and Mozc-derived IMEs (see [Roadmap](#roadmap-mozc-derived-imes)).

## Build, run, test

```bash
cargo build --release
./target/release/mozcli restart
./target/release/mozcli --lang en restart   # or -l ja
LANG=ja_JP.UTF-8 ./target/release/mozcli restart

python tests/test_i18n.py                   # or: pytest tests/
```

There are no Rust unit tests yet. `clap` is pinned to `=4.4.18`; do not bump it casually.

## Repository layout

```
mozcli/
├── AGENTS.md          # This file
├── CLAUDE.md          # Claude Code entry point (imports this file)
├── Cargo.toml         # package / binary name: mozcli
├── README.md
├── src/main.rs        # All application logic (single file)
├── locales/
│   ├── en.json        # English messages
│   └── ja.json        # Japanese messages
└── tests/
    └── test_i18n.py   # Locale file checks (Python)
```

## How the code works (`src/main.rs`)

### CLI

- `Args` (clap derive): one positional `command` (`CommandType` value enum) plus `--lang` / `-l`.
- Commands: `restart` (implemented), `status` (prints "not implemented", exits 1).

### i18n

- `Translations` is a struct with **one field per message key**. Adding a key means updating:
  1. the `Translations` struct,
  2. `FALLBACK_EN`,
  3. the `match` in `t()`,
  4. `locales/en.json` **and** `locales/ja.json`.
  A missing key in a JSON file makes `serde_json` reject the whole file, and that language silently falls back to English.
- `find_locales_dir()` looks for `locales/` next to the executable, then in the current directory, then at `CARGO_MANIFEST_DIR` (build-time path). Locale files are **not** embedded in the binary.
- `t(key, lang, vars)` replaces `{name}` placeholders with values from `vars`. Unknown keys return the key itself.
- `detect_lang()`: `--lang` (anything starting with `ja` → `ja`, else `en`), then `LC_ALL` / `LC_MESSAGES` / `LANG`, else `en`. The `#[cfg(windows)]` block re-checks `LANG` only; real Windows UI language detection is not implemented.
- Keys `title`, `command_restart`, `command_status`, `error` exist in the locale files but are not printed anywhere yet.

### Restart logic

- `restart_mozc()` dispatches on `std::env::consts::OS`; other OSes print `unsupported_os` and exit 1.
- **Windows** (`restart_windows`): `taskkill /F /IM <name> /T` for
  `GoogleIMEJaConverter.exe`, `GoogleIMEJaRenderer.exe`, `GoogleJapaneseInput.exe`, then sleeps 300 ms. It does not start anything again; the user is told to toggle the IME (Mozc relaunches the server on demand).
- **macOS** (`restart_macos`): runs `launchctl list | grep -E 'com\.google\.inputmethod\.Japanese\.(Converter|Renderer)'`, then `launchctl stop` / `start` on each label, 200 ms apart.
- `kill_process()` uses `taskkill` on Windows and `pkill -f` elsewhere. The `pkill` path is currently unused (macOS uses `launchctl`).

### Known gaps

- Only Google Japanese Input names are hard-coded; OSS Mozc builds are not handled.
- Exit code is 0 even when nothing was killed or no services were found; command failures are ignored (`let _ = ...`).
- `restart_complete` is printed even when nothing was restarted.
- The macOS path depends on `sh`, `grep`, and `awk`.

## Roadmap: Mozc-derived IMEs

Goal: `mozcli start|stop|restart [--ime <id>]` across Mozc and its derivatives.

Suggested design (keep it simple):

1. **IME profile table.** Replace the hard-coded lists with a static table, e.g.
   ```rust
   struct ImeProfile {
       id: &'static str,                  // "google", "mozc", ...
       display_name: &'static str,
       windows_processes: &'static [&'static str],
       macos_launchd_labels: &'static [&'static str], // or a label prefix
   }
   ```
   A new derivative should only need a new table entry.
2. **Commands.** Add `start` and `stop` to `CommandType`; implement `restart` as stop + start. Keep `status` on the roadmap.
3. **IME selection.** `--ime <id>` picks a profile; with no flag, detect which profiles are installed or running and act on those.
4. **Exit codes.** Return non-zero when nothing matched or a command failed. Print errors through `t("error", ...)`.
5. **Windows start.** Decide how to start the server explicitly (for example, by launching the converter executable from its install directory) and verify it against a real install before relying on it.

Process names and launchd labels for any IME other than Google Japanese Input are **unverified**. Confirm them on a real installation (Task Manager / `tasklist`, `launchctl list`, the `.app` bundle's `Info.plist`) before adding them, and note in the PR how they were checked.

## Coding guidelines

- Production code is **Rust**; Python is fine for tests and prototypes.
- Keep it a single, self-contained binary. Avoid heavy dependencies.
- Every code path must work on both Windows and macOS. Use `#[cfg(...)]` or `std::env::consts::OS` and keep both branches compiling.
- All user-facing text goes through `t()`. Do not hard-code English or Japanese strings in source.
- Locale files stay flat JSON key/value pairs. No i18n frameworks.
- When adding a key, update both `en.json` and `ja.json` (`tests/test_i18n.py` checks that the key sets match).
- Keep the tool minimal. Do not add features that were not requested unless they clearly improve reliability or usability.

## Communication

- The author prefers technical discussion in **Japanese**.
- Documentation and code comments are written in **English**.
