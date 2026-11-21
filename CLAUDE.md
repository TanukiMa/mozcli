# CLAUDE.md

Project guidance lives in AGENTS.md so that all agents share one source of truth:

@AGENTS.md

## Claude Code notes

- Reply to the author in Japanese; write code, comments, and docs in English.
- After changing `src/main.rs`, run `cargo build`. After changing `locales/`, run `python tests/test_i18n.py`.
- Real IME process control (`taskkill`, `launchctl`) affects the user's live input method. Do not run `mozcli restart|stop|start` without asking.
- When code behavior changes, update the "How the code works" and "Known gaps" sections of AGENTS.md in the same change.
