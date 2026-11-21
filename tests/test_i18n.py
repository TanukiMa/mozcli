#!/usr/bin/env python3
"""
Simple tests for locale files (can be expanded later).
These are intentionally written in Python as agreed in the design discussion.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).parent.parent
LOCALES_DIR = ROOT / "locales"
MAIN_RS = ROOT / "src" / "main.rs"


def test_locale_files_exist():
    assert (LOCALES_DIR / "en.json").exists()
    assert (LOCALES_DIR / "ja.json").exists()


def test_locale_files_are_valid_json():
    for lang in ["en", "ja"]:
        with open(LOCALES_DIR / f"{lang}.json", encoding="utf-8") as f:
            data = json.load(f)
            assert isinstance(data, dict)
            assert "restart_begin" in data
            assert "restart_complete" in data


def test_keys_are_consistent():
    with open(LOCALES_DIR / "en.json", encoding="utf-8") as f:
        en = set(json.load(f).keys())
    with open(LOCALES_DIR / "ja.json", encoding="utf-8") as f:
        ja = set(json.load(f).keys())
    assert en == ja, f"Key mismatch: en-only={en-ja}, ja-only={ja-en}"


def test_keys_used_in_source_exist():
    """Every message key referenced in main.rs must exist in all locale files."""
    src = MAIN_RS.read_text(encoding="utf-8").split("#[cfg(test)]")[0]
    # Keys passed directly to t("...").
    used = set(re.findall(r'\bt\(\s*"([a-z_]+)"', src))
    # Keys passed indirectly: ("x_begin", "x_complete") pairs and mac_launchctl(..., "key", ...).
    for begin, done in re.findall(r'\("([a-z_]+_begin)",\s*"([a-z_]+_complete)"\)', src):
        used |= {begin, done}
    used |= set(re.findall(r'mac_launchctl\(profile,\s*"[a-z]+",\s*"([a-z_]+)"', src))
    assert used, "no keys found; the regexes are out of date"
    for lang in ["en", "ja"]:
        with open(LOCALES_DIR / f"{lang}.json", encoding="utf-8") as f:
            keys = set(json.load(f).keys())
        missing = used - keys
        assert not missing, f"{lang}.json is missing keys used in main.rs: {missing}"


if __name__ == "__main__":
    test_locale_files_exist()
    test_locale_files_are_valid_json()
    test_keys_are_consistent()
    test_keys_used_in_source_exist()
    print("All locale tests passed.")
