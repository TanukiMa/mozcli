use clap::{Parser, ValueEnum};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::Duration;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "mozcli")]
#[command(author, version, about = "CLI tool to control Mozc-based Japanese IMEs")]
struct Args {
    #[arg(value_enum)]
    command: CommandType,

    /// Target IME id (e.g. "google"). Default: every installed IME.
    #[arg(short, long)]
    ime: Option<String>,

    /// Force language (ja / en). Overrides system locale.
    #[arg(short, long)]
    lang: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum CommandType {
    /// Start the IME server
    Start,
    /// Stop the IME server and renderer
    Stop,
    /// Stop, then start the IME
    Restart,
    /// Show status of IME processes (not yet implemented)
    Status,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Action {
    Start,
    Stop,
    Restart,
}

// ---------------------------------------------------------------------------
// IME profiles
// ---------------------------------------------------------------------------

/// Everything mozcli needs to know about one Mozc-based IME.
/// Supporting a new derivative should only require a new entry in `PROFILES`.
/// Only add names that were verified on a real installation.
struct ImeProfile {
    /// Value accepted by `--ime`.
    id: &'static str,
    display_name: &'static str,
    /// Install directory, relative to `%ProgramFiles(x86)%` or `%ProgramFiles%`.
    windows_install_subdir: &'static str,
    /// Main server executable, used to detect installation and running state.
    /// Must also appear in `windows_processes`.
    windows_server_exe: &'static str,
    /// Processes terminated by `stop`.
    windows_processes: &'static [&'static str],
    /// launchd labels controlled with `launchctl stop` / `start`.
    macos_launchd_labels: &'static [&'static str],
}

static PROFILES: &[ImeProfile] = &[ImeProfile {
    id: "google",
    display_name: "Google Japanese Input",
    windows_install_subdir: r"Google\Google Japanese Input",
    windows_server_exe: "GoogleIMEJaConverter.exe",
    windows_processes: &["GoogleIMEJaConverter.exe", "GoogleIMEJaRenderer.exe"],
    macos_launchd_labels: &[
        "com.google.inputmethod.Japanese.Converter",
        "com.google.inputmethod.Japanese.Renderer",
    ],
}];

fn find_profile(id: &str) -> Option<&'static ImeProfile> {
    PROFILES.iter().find(|p| p.id.eq_ignore_ascii_case(id))
}

// ---------------------------------------------------------------------------
// i18n
// ---------------------------------------------------------------------------

/// Locale files are embedded at build time so the binary is self-contained.
static LOCALES: Lazy<HashMap<&'static str, HashMap<String, String>>> = Lazy::new(|| {
    [
        ("en", include_str!("../locales/en.json")),
        ("ja", include_str!("../locales/ja.json")),
    ]
    .into_iter()
    .map(|(lang, src)| {
        let map: HashMap<String, String> = serde_json::from_str(src)
            .unwrap_or_else(|e| panic!("locales/{lang}.json is invalid: {e}"));
        (lang, map)
    })
    .collect()
});

/// Look up `key` for `lang` (falling back to English, then to the key itself)
/// and replace `{name}` placeholders with `vars`.
fn t(key: &str, lang: &str, vars: &[(&str, &str)]) -> String {
    let mut text = LOCALES
        .get(lang)
        .and_then(|m| m.get(key))
        .or_else(|| LOCALES.get("en").and_then(|m| m.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string());

    for (k, v) in vars {
        text = text.replace(&format!("{{{}}}", k), v);
    }
    text
}

fn detect_lang(forced: Option<&str>) -> String {
    if let Some(lang) = forced {
        if lang.to_lowercase().starts_with("ja") {
            return "ja".to_string();
        }
        return "en".to_string();
    }

    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(var) {
            if val.to_lowercase().starts_with("ja") {
                return "ja".to_string();
            }
        }
    }

    "en".to_string()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_quiet(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

fn win_install_dir(profile: &ImeProfile) -> Option<PathBuf> {
    ["ProgramFiles(x86)", "ProgramFiles"]
        .iter()
        .filter_map(|var| env::var_os(var))
        .map(|base| PathBuf::from(base).join(profile.windows_install_subdir))
        .find(|dir| dir.join(profile.windows_server_exe).is_file())
}

/// `tasklist` CSV output contains `"<image name>",...` when the process exists.
/// (The "not found" message is localized, so only the positive case is matched.)
fn tasklist_contains(output: &str, image: &str) -> bool {
    let needle = format!("\"{}\"", image.to_lowercase());
    output.to_lowercase().contains(&needle)
}

fn win_is_running(image: &str) -> bool {
    let filter = format!("IMAGENAME eq {image}");
    Command::new("tasklist")
        .args(["/FI", &filter, "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|out| tasklist_contains(&String::from_utf8_lossy(&out.stdout), image))
        .unwrap_or(false)
}

fn win_stop(profile: &ImeProfile, lang: &str) -> bool {
    let mut ok = true;
    for &proc in profile.windows_processes {
        if !win_is_running(proc) {
            println!("{}", t("not_running", lang, &[("proc", proc)]));
        } else if run_quiet("taskkill", &["/F", "/IM", proc, "/T"]) {
            println!("{}", t("killed", lang, &[("proc", proc)]));
        } else {
            println!("{}", t("command_failed", lang, &[("target", proc)]));
            ok = false;
        }
    }
    ok
}

/// Windows has no explicit start: launching the converter directly makes it
/// exit with 0xC0000409 after ~3 s (verified 2026-11-21, also from a normal
/// terminal). The IME client (TIP DLL inside each application) relaunches the
/// converter and renderer on the next input, so `start` only checks the state.
fn win_start(profile: &ImeProfile, lang: &str) -> bool {
    let server = profile.windows_server_exe;
    if win_is_running(server) {
        println!("{}", t("already_running", lang, &[("proc", server)]));
        return true;
    }

    if win_install_dir(profile).is_none() {
        println!(
            "{}",
            t("not_installed", lang, &[("ime", profile.display_name)])
        );
        return false;
    }

    println!("{}", t("on_demand", lang, &[("proc", server)]));
    true
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

/// Extract labels (third column) from `launchctl list` output.
fn parse_launchctl_list(output: &str) -> HashSet<String> {
    output
        .lines()
        .filter_map(|line| line.split_whitespace().nth(2))
        .map(str::to_string)
        .collect()
}

fn mac_loaded_labels() -> HashSet<String> {
    Command::new("launchctl")
        .arg("list")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| parse_launchctl_list(&String::from_utf8_lossy(&out.stdout)))
        .unwrap_or_default()
}

fn mac_profile_labels(profile: &ImeProfile, loaded: &HashSet<String>) -> Vec<&'static str> {
    profile
        .macos_launchd_labels
        .iter()
        .copied()
        .filter(|label| loaded.contains(*label))
        .collect()
}

/// Run `launchctl <verb>` on every loaded label of the profile.
fn mac_launchctl(profile: &ImeProfile, verb: &str, done_key: &str, lang: &str) -> bool {
    let labels = mac_profile_labels(profile, &mac_loaded_labels());
    if labels.is_empty() {
        println!("{}", t("no_services_found", lang, &[]));
        return false;
    }

    let mut ok = true;
    for label in labels {
        if run_quiet("launchctl", &[verb, label]) {
            println!("{}", t(done_key, lang, &[("service", label)]));
        } else {
            println!("{}", t("command_failed", lang, &[("target", label)]));
            ok = false;
        }
    }
    ok
}

fn mac_stop(profile: &ImeProfile, lang: &str) -> bool {
    mac_launchctl(profile, "stop", "stopped", lang)
}

fn mac_start(profile: &ImeProfile, lang: &str) -> bool {
    mac_launchctl(profile, "start", "started", lang)
}

// ---------------------------------------------------------------------------
// Platform dispatch
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, PartialEq, Eq)]
enum Platform {
    Windows,
    MacOs,
}

impl Platform {
    fn current() -> Option<Self> {
        match env::consts::OS {
            "windows" => Some(Platform::Windows),
            "macos" => Some(Platform::MacOs),
            _ => None,
        }
    }

    fn is_installed(self, profile: &ImeProfile) -> bool {
        match self {
            Platform::Windows => win_install_dir(profile).is_some(),
            Platform::MacOs => !mac_profile_labels(profile, &mac_loaded_labels()).is_empty(),
        }
    }

    fn stop(self, profile: &ImeProfile, lang: &str) -> bool {
        match self {
            Platform::Windows => win_stop(profile, lang),
            Platform::MacOs => mac_stop(profile, lang),
        }
    }

    fn start(self, profile: &ImeProfile, lang: &str) -> bool {
        match self {
            Platform::Windows => win_start(profile, lang),
            Platform::MacOs => mac_start(profile, lang),
        }
    }
}

fn run_action(platform: Platform, action: Action, profile: &ImeProfile, lang: &str) -> bool {
    let os = env::consts::OS;
    let ime = profile.display_name;
    let (begin_key, done_key) = match action {
        Action::Start => ("start_begin", "start_complete"),
        Action::Stop => ("stop_begin", "stop_complete"),
        Action::Restart => ("restart_begin", "restart_complete"),
    };
    println!("{}", t(begin_key, lang, &[("os", os), ("ime", ime)]));

    let ok = match action {
        Action::Start => platform.start(profile, lang),
        Action::Stop => platform.stop(profile, lang),
        Action::Restart => {
            let stopped = platform.stop(profile, lang);
            thread::sleep(Duration::from_millis(300));
            // Start even if stopping partly failed, so the IME is not left down.
            let started = platform.start(profile, lang);
            stopped && started
        }
    };

    if !ok {
        return false;
    }

    match (platform, action) {
        (_, Action::Stop) => println!("{}", t("stop_note", lang, &[])),
        (Platform::Windows, _) => println!("{}", t("win_restart_note", lang, &[])),
        (Platform::MacOs, _) => println!("{}", t("mac_restart_note", lang, &[])),
    }
    println!("{}", t(done_key, lang, &[("ime", ime)]));
    true
}

fn main() -> ExitCode {
    let args = Args::parse();
    let lang = detect_lang(args.lang.as_deref());

    let action = match args.command {
        CommandType::Start => Action::Start,
        CommandType::Stop => Action::Stop,
        CommandType::Restart => Action::Restart,
        CommandType::Status => {
            println!("{}", t("status_not_implemented", &lang, &[]));
            return ExitCode::FAILURE;
        }
    };

    let Some(platform) = Platform::current() else {
        println!("{}", t("unsupported_os", &lang, &[]));
        return ExitCode::FAILURE;
    };

    let targets: Vec<&ImeProfile> = match args.ime.as_deref() {
        Some(id) => match find_profile(id) {
            Some(p) => vec![p],
            None => {
                let available: Vec<&str> = PROFILES.iter().map(|p| p.id).collect();
                let msg = t(
                    "unknown_ime",
                    &lang,
                    &[("ime", id), ("available", &available.join(", "))],
                );
                eprintln!("{}", t("error", &lang, &[("msg", &msg)]));
                return ExitCode::FAILURE;
            }
        },
        None => PROFILES
            .iter()
            .filter(|p| platform.is_installed(p))
            .collect(),
    };

    if targets.is_empty() {
        eprintln!("{}", t("no_ime_found", &lang, &[]));
        return ExitCode::FAILURE;
    }

    let mut all_ok = true;
    for profile in targets {
        all_ok &= run_action(platform, action, profile, &lang);
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_have_same_keys() {
        let en: HashSet<_> = LOCALES["en"].keys().collect();
        let ja: HashSet<_> = LOCALES["ja"].keys().collect();
        assert_eq!(en, ja);
    }

    #[test]
    fn t_replaces_placeholders_and_falls_back() {
        assert_eq!(
            t("killed", "en", &[("proc", "a.exe")]),
            "  Stopped process: a.exe"
        );
        assert_eq!(t("no_such_key", "ja", &[]), "no_such_key");
        assert_eq!(t("killed", "xx", &[("proc", "a")]), t("killed", "en", &[("proc", "a")]));
    }

    #[test]
    fn profiles_are_consistent() {
        let mut ids = HashSet::new();
        for p in PROFILES {
            assert!(ids.insert(p.id), "duplicate id {}", p.id);
            assert!(p.windows_processes.contains(&p.windows_server_exe));
        }
        assert!(find_profile("Google").is_some());
        assert!(find_profile("nope").is_none());
    }

    #[test]
    fn parses_launchctl_list() {
        let out = "PID\tStatus\tLabel\n\
                   123\t0\tcom.google.inputmethod.Japanese.Converter\n\
                   -\t0\tcom.apple.something\n";
        let labels = parse_launchctl_list(out);
        assert!(labels.contains("com.google.inputmethod.Japanese.Converter"));
        assert!(labels.contains("com.apple.something"));

        let google = find_profile("google").unwrap();
        assert_eq!(
            mac_profile_labels(google, &labels),
            vec!["com.google.inputmethod.Japanese.Converter"]
        );
    }

    #[test]
    fn detects_tasklist_match() {
        let found = "\"GoogleIMEJaConverter.exe\",\"13728\",\"Console\",\"1\",\"45,220 K\"\r\n";
        assert!(tasklist_contains(found, "GoogleIMEJaConverter.exe"));
        assert!(!tasklist_contains("INFO: No tasks are running...", "GoogleIMEJaConverter.exe"));
    }
}
