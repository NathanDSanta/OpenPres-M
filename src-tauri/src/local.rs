//! This computer's server: whether it's used as one, whether its server runs, and starting
//! it. The server is its own program (from the OpenPres repository); the Manager only looks
//! at its files, asks it over HTTP and starts it with `--server`.

use crate::paths;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The server's port when its settings don't say (the server's `DEFAULT_API_PORT`).
pub const DEFAULT_PORT: u16 = 8787;

/// Where people get the server when it isn't installed here.
pub const DOWNLOAD_PAGE: &str = "https://github.com/NathanDSanta/OpenPres/releases/latest";

const THIS_COMPUTER_FILE: &str = "this-computer.json";
/// The server writes where its program is each time it starts (the AppImage's file on
/// Linux, a folder chosen in the installer on Windows…).
const SERVER_PROGRAM_FILE: &str = "server-program.txt";

/// What the Manager can see of this computer's server without asking it: its port, whether
/// a server has run here (its settings exist) and whether it's set up (both passwords).
#[derive(Debug, PartialEq, Eq)]
pub struct Peek {
    pub port: u16,
    pub has_data: bool,
    pub set_up: bool,
}

pub fn peek(config: &Path) -> Peek {
    let json = std::fs::read_to_string(config)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let port = json
        .as_ref()
        .and_then(|j| j.get("api_port")?.as_u64())
        .and_then(|p| u16::try_from(p).ok())
        .or_else(|| std::env::var("OPENPRES_PORT").ok()?.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let has = |key: &str| json.as_ref().is_some_and(|j| j.get(key).is_some_and(|v| !v.is_null()));
    Peek {
        port,
        has_data: config.exists(),
        set_up: has("admin_password_hash") && has("password_hash"),
    }
}

/// Whether this computer is used as a server (chosen when the Manager first opened, later in
/// its server list, or by the server's installer). None: not chosen yet.
pub fn this_computer_is_server() -> Option<bool> {
    choice_in(&paths::root())
}

pub fn set_this_computer_is_server(on: bool) -> std::io::Result<()> {
    set_choice_in(&paths::root(), on)
}

fn choice_in(root: &Path) -> Option<bool> {
    let text = std::fs::read_to_string(root.join(THIS_COMPUTER_FILE)).ok()?;
    serde_json::from_str::<serde_json::Value>(&text).ok()?.get("server")?.as_bool()
}

fn set_choice_in(root: &Path, on: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join(THIS_COMPUTER_FILE), format!("{{\n  \"server\": {on}\n}}\n"))
}

pub fn host_name() -> String {
    let name = gethostname::gethostname().to_string_lossy().into_owned();
    let name = name.split('.').next().unwrap_or("").trim().to_string();
    if name.is_empty() {
        "OpenPres".into()
    } else {
        name
    }
}

/// The server's program, if it's installed here: where the server last said it is, or where
/// its installers put it.
pub fn server_program() -> Option<PathBuf> {
    candidates(&paths::root()).into_iter().find(|p| p.is_file())
}

fn candidates(root: &Path) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if let Some(p) = std::env::var_os("OPENPRES_SERVER_PROGRAM") {
        list.push(PathBuf::from(p));
    }
    if let Ok(text) = std::fs::read_to_string(root.join(SERVER_PROGRAM_FILE)) {
        let line = text.lines().next().unwrap_or("").trim();
        if !line.is_empty() {
            list.push(PathBuf::from(line));
        }
    }
    list.extend(installed_places());
    list
}

#[cfg(windows)]
fn installed_places() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;
    const NO_WINDOW: u32 = 0x0800_0000;
    let mut list = Vec::new();
    // The installer records its folder (also one the person chose) for "Apps & features".
    for hive in ["HKCU", "HKLM"] {
        let out = std::process::Command::new("reg")
            .args(["query", &format!(r"{hive}\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenPres"), "/v", "InstallLocation"])
            .creation_flags(NO_WINDOW)
            .output();
        if let Some(dir) = out.ok().and_then(|o| install_location(&String::from_utf8_lossy(&o.stdout))) {
            list.push(dir.join("openpres.exe"));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        list.push(PathBuf::from(local).join("OpenPres").join("openpres.exe"));
    }
    list
}

#[cfg(target_os = "macos")]
fn installed_places() -> Vec<PathBuf> {
    let app = PathBuf::from("OpenPres.app/Contents/MacOS/openpres");
    let mut list = vec![PathBuf::from("/Applications").join(&app)];
    if let Some(home) = std::env::var_os("HOME") {
        list.push(PathBuf::from(home).join("Applications").join(&app));
    }
    list
}

#[cfg(not(any(windows, target_os = "macos")))]
fn installed_places() -> Vec<PathBuf> {
    // The .deb and .rpm put it on the PATH; an AppImage says where it is when it runs.
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).map(|dir| dir.join("openpres")).collect())
        .unwrap_or_default()
}

/// The folder in `reg query`'s answer (`InstallLocation    REG_SZ    "C:\…\OpenPres"`).
#[cfg_attr(not(windows), allow(dead_code))]
fn install_location(reg_output: &str) -> Option<PathBuf> {
    let line = reg_output.lines().find(|l| l.trim_start().starts_with("InstallLocation"))?;
    let value = line.split("REG_SZ").nth(1)?.trim().trim_matches('"');
    (!value.is_empty()).then(|| PathBuf::from(value))
}

/// Starts the server in the background (it keeps running after the Manager closes).
pub fn start_server() -> Result<(), String> {
    let program = server_program().ok_or_else(|| {
        format!("The OpenPres server isn't installed on this computer. Download it from {DOWNLOAD_PAGE}, install it, and try again.")
    })?;
    tracing::info!("starting the server: {}", program.display());
    std::process::Command::new(&program).arg("--server").spawn().map(|_| ()).map_err(|e| {
        tracing::error!("could not start the server ({}): {e}", program.display());
        format!("Couldn't start the server ({}): {e}", program.display())
    })
}

/// What the server list shows about this computer.
#[derive(Serialize)]
pub struct LocalStatus {
    /// A server runs on this computer.
    running: bool,
    port: u16,
    /// What it says about itself (`/api/server`), when it runs.
    info: Option<serde_json::Value>,
    /// This computer's name.
    host: String,
    /// This computer has been a server before: the Manager starts it by itself.
    has_data: bool,
    /// Used as a server (chosen when the Manager first opened); None: not chosen yet.
    choice: Option<bool>,
    /// Its server has passwords (set up).
    set_up: bool,
    /// The server's program is installed here.
    installed: bool,
}

/// Is a server running on this computer? (Asked straight, so it's found even when it
/// doesn't accept other devices and isn't announced.)
pub async fn status() -> LocalStatus {
    let peek = peek(&paths::server_config_file());
    let info = async {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(800))
            .build()
            .ok()?
            .get(format!("http://127.0.0.1:{}/api/server", peek.port))
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()
    }
    .await;
    LocalStatus {
        running: info.is_some(),
        port: peek.port,
        info,
        host: host_name(),
        has_data: peek.has_data,
        choice: this_computer_is_server(),
        set_up: peek.set_up,
        installed: server_program().is_some(),
    }
}

/// Uses this computer as a server, or not. Not: its server stops (if it runs); when it next
/// starts with the computer it sees the choice, takes itself off the start-up list and
/// quits. Its songs and settings stay for later.
pub async fn set_this_computer(server: bool) -> Result<(), String> {
    tracing::info!("this computer {} a server", if server { "is" } else { "isn't" });
    set_this_computer_is_server(server).map_err(|e| e.to_string())?;
    if !server {
        let port = peek(&paths::server_config_file()).port;
        let _ = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?
            .post(format!("http://127.0.0.1:{port}/api/server/quit"))
            .send()
            .await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_is_kept_in_a_file_the_server_reads_too() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(choice_in(dir.path()), None, "not chosen yet");
        set_choice_in(dir.path(), false).unwrap();
        assert_eq!(choice_in(dir.path()), Some(false));
        set_choice_in(dir.path(), true).unwrap();
        assert_eq!(choice_in(dir.path()), Some(true));
        // The server's format.
        assert_eq!(std::fs::read_to_string(dir.path().join(THIS_COMPUTER_FILE)).unwrap(), "{\n  \"server\": true\n}\n");
    }

    #[test]
    fn peeks_at_the_servers_settings_without_changing_them() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("config.json");
        assert_eq!(peek(&file), Peek { port: DEFAULT_PORT, has_data: false, set_up: false });

        std::fs::write(&file, r#"{"api_port": 9001, "admin_password_hash": "x", "password_hash": null}"#).unwrap();
        assert_eq!(peek(&file), Peek { port: 9001, has_data: true, set_up: false }, "no operator password yet");

        std::fs::write(&file, r#"{"api_port": 9001, "admin_password_hash": "x", "password_hash": "y"}"#).unwrap();
        assert!(peek(&file).set_up);

        std::fs::write(&file, "not json").unwrap();
        assert_eq!(peek(&file), Peek { port: DEFAULT_PORT, has_data: true, set_up: false });
    }

    #[test]
    fn the_servers_own_note_of_its_program_comes_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(SERVER_PROGRAM_FILE), "/opt/OpenPres.AppImage\n").unwrap();
        let list = candidates(dir.path());
        let at = list.iter().position(|p| p == Path::new("/opt/OpenPres.AppImage")).unwrap();
        assert!(at <= 1, "only OPENPRES_SERVER_PROGRAM goes before it: {list:?}");
        assert!(list.len() > at + 1, "then where the installers put it");
    }

    #[test]
    fn reads_the_install_folder_from_reg_query() {
        let out = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenPres\r\n    InstallLocation    REG_SZ    \"C:\\Users\\Ana María\\AppData\\Local\\OpenPres\"\r\n\r\n";
        assert_eq!(install_location(out), Some(PathBuf::from("C:\\Users\\Ana María\\AppData\\Local\\OpenPres")));
        assert_eq!(install_location("ERROR: The system was unable to find the specified registry key or value."), None);
    }
}
