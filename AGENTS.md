# AGENTS.md — mozcli

Guidance for contributors and AI coding agents. Keep this file in sync with the code.

## Project overview

`mozcli` (formerly `mozc-cli`) is a Rust command-line tool for controlling Mozc-based Japanese IMEs on **Windows** and **macOS**.

It started from a request by a Mozc developer (yukawa):

> "We want a Mozc CLI. For now, a tool that can restart `mozc_server`, `mozc_renderer`, and the client on Windows and macOS would be good."

**Direction:** `start` / `stop` / `restart` for Mozc and Mozc-derived IMEs. Each IME is described by one entry in a profile table (see [Adding an IME](#adding-an-ime)).

## Build, run, test

```bash
cargo build --release
./target/release/mozcli restart               # every installed IME
./target/release/mozcli --ime google stop     # one IME
./target/release/mozcli -l en start           # force English (or -l ja)

cargo test                                    # Rust unit tests
python tests/test_i18n.py                     # locale checks (or: pytest tests/)
```

`clap` is pinned to `=4.4.18`; do not bump it casually.

## Repository layout

```
mozcli/
├── AGENTS.md          # This file
├── CLAUDE.md          # Claude Code entry point (imports this file)
├── Cargo.toml         # package / binary name: mozcli
├── README.md
├── src/main.rs        # All application logic and Rust unit tests (single file)
├── locales/
│   ├── en.json        # English messages (embedded at build time)
│   └── ja.json        # Japanese messages (embedded at build time)
└── tests/
    └── test_i18n.py   # Locale file checks (Python)
```

## How the code works (`src/main.rs`)

### CLI

- `Args` (clap derive): positional `command` (`CommandType`), `--ime` / `-i`, `--lang` / `-l`.
- Commands: `start`, `stop`, `restart`, and `status` (prints "not implemented", exits 1).
- `CommandType` is mapped to the internal `Action` enum (`Start` / `Stop` / `Restart`).

### IME profiles

- `ImeProfile` describes one IME: `id` (the `--ime` value), `display_name`, `windows_install_subdir`, `windows_server_exe`, `windows_processes`, `macos_launchd_labels`.
- `PROFILES` is the static table. It currently has only `google` (Google Japanese Input).
- `find_profile()` matches `--ime` case-insensitively.
- Target selection in `main()`:
  - with `--ime`: that profile only; an unknown id prints `unknown_ime` and exits 1;
  - without `--ime`: every profile for which `Platform::is_installed()` is true; if none, prints `no_ime_found` and exits 1.

### Platform layer

- `Platform::current()` maps `std::env::consts::OS` to `Windows` / `MacOs`; other OSes print `unsupported_os` and exit 1.
- The Windows and macOS functions are **not** behind `#[cfg]`, so both compile (and are type-checked) on every OS; dispatch happens at runtime.
- **Windows**
  - `win_install_dir()`: first of `%ProgramFiles(x86)%\<subdir>` and `%ProgramFiles%\<subdir>` that contains `windows_server_exe`. This is also the "installed" check.
  - `win_is_running()`: `tasklist /FI "IMAGENAME eq <exe>" /NH /FO CSV`, matched by `tasklist_contains()` (looks for `"<exe>"`, since the not-found message is localized).
  - `win_stop()`: for each running process, `taskkill /F /IM <exe> /T`. A process that is not running is reported and is not an error.
  - `win_start()`: **does not launch anything.** If the server is running it prints `already_running`; if the IME is installed it prints `on_demand` and succeeds; otherwise `not_installed` and fails. The IME client (the TIP DLL loaded in each application) relaunches the converter and renderer on the next input. Launching the converter directly does not work (see [Verified behavior](#verified-behavior)).
- **macOS**
  - `mac_loaded_labels()`: runs `launchctl list` and parses the label column (`parse_launchctl_list()`).
  - A profile counts as installed when at least one of its `macos_launchd_labels` is loaded.
  - `mac_stop()` / `mac_start()`: `launchctl stop|start <label>` for each loaded label. If no label is loaded, prints `no_services_found` and fails.
- `run_action()`: prints `<action>_begin`, runs the action (restart = stop, 300 ms sleep, start; start runs even if stop partly failed), then on success prints a note (`stop_note`, `win_restart_note`, or `mac_restart_note`) and `<action>_complete`.

### Exit codes

- `0`: every targeted IME succeeded.
- `1`: unknown IME, no IME found, unsupported OS, `status`, or any command failure.
- `2`: argument parse error (from clap).

### i18n

- `locales/*.json` are flat `{ "key": "text" }` maps, embedded with `include_str!` into `LOCALES` (a `HashMap` per language). No runtime file lookup.
- `t(key, lang, vars)`: looks up `lang`, then `en`, then returns the key itself; replaces `{name}` placeholders from `vars`.
- Adding a message: add the key to **both** `en.json` and `ja.json`, then call `t("key", ...)`. No struct to update.
- `detect_lang()`: `--lang` (anything starting with `ja` → `ja`, else `en`), then `LC_ALL` / `LC_MESSAGES` / `LANG`, else `en`. Windows UI language detection is not implemented.
- `title` is defined but not printed anywhere.

### Tests

- Rust (`cargo test`): locale key sets match, `t()` substitution and fallback, profile table consistency, `launchctl list` parsing, `tasklist` matching. Tests never run `taskkill` / `launchctl`.
- Python (`tests/test_i18n.py`): locale files exist and are valid, key sets match, and every key referenced in `main.rs` exists. The key scan uses regexes over the non-test part of `main.rs`; if you pass keys in a new indirect way, extend the regexes.

### Known gaps

- **Windows `start` / `restart` do not bring the server up by themselves**; they rely on the client relaunching it on the next input.
- **macOS code has not been run on a real Mac** since the rewrite (it only compiles on Windows here).
- After `stop`, applications that use the IME start the server again on the next input; `stop` is temporary.
- `launchctl stop` / `start` are legacy subcommands; `kickstart` / `bootout` are not used.
- Only Google Japanese Input is in `PROFILES`; OSS Mozc builds and other derivatives are not.
- `status` is not implemented.

## Adding an IME

1. Collect, on a real installation:
   - Windows: install directory, and process names (`tasklist`, Task Manager);
   - macOS: launchd labels (`launchctl list`, `/Library/LaunchAgents/*.plist`, the `.app` bundle's `Info.plist`).
2. Add one `ImeProfile` entry to `PROFILES`. Leave a list empty (`&[]`) if the IME is not available on that OS.
3. Run `cargo test` and `python tests/test_i18n.py`.
4. In the PR, state how and on which version the names were verified.

Do not add names from memory or from the Mozc source tree alone; branding-specific names differ between builds.

## Verified names

| IME | OS | Item | Value | How verified |
|-----|----|------|-------|--------------|
| google | Windows | install dir | `C:\Program Files (x86)\Google\Google Japanese Input` | directory listing, 2026-11-21 |
| google | Windows | processes | `GoogleIMEJaConverter.exe`, `GoogleIMEJaRenderer.exe` | `tasklist`, 2026-11-21 |
| google | macOS | launchd labels | `com.google.inputmethod.Japanese.Converter` / `.Renderer` | carried over from v0.1, not re-checked |

### Verified behavior

Google Japanese Input on Windows 11, 2026-11-21:

- `taskkill /F /IM GoogleIMEJaConverter.exe` / `GoogleIMEJaRenderer.exe` stops both processes.
- After a stop, the client relaunches both processes on the next input; their parent process is the host application (observed: `WindowsTerminal.exe`).
- Launching `GoogleIMEJaConverter.exe` directly (no arguments, cwd = install dir) exits after ~3 s with `0xC0000409`. Reproduced from Claude Code and from a separate normal `cmd` terminal, so it is not caused by the agent environment. Do not reintroduce a direct launch without a new finding.

### Notes on names

`GoogleJapaneseInput.exe` (in v0.1) does not exist in the Windows install and was dropped. `GoogleIMEJaCacheService.exe` is a Windows service and `GoogleIMEJaBroker.exe` / `GoogleIMEJaTool.exe` are helper tools; mozcli does not touch them.

## Coding guidelines

- Production code is **Rust**; Python is fine for tests and prototypes.
- Keep it a single, self-contained binary. Avoid heavy dependencies.
- Every code path must work on both Windows and macOS, and both must compile on either OS.
- All user-facing text goes through `t()`. Do not hard-code English or Japanese strings in source.
- Locale files stay flat JSON key/value pairs. No i18n frameworks.
- Keep the tool minimal. Do not add features that were not requested unless they clearly improve reliability or usability.

## Communication

- The author prefers technical discussion in **Japanese**.
- Documentation and code comments are written in **English**.
