//! Where the Manager keeps its files, next to the server's (the server's own program,
//! from the OpenPres repository, keeps them in `Server/`):
//!
//! ```text
//! <data>/OpenPres/              Windows: %APPDATA%\OpenPres
//! ├── this-computer.json        is this computer a server? (shared with the server)
//! ├── server-program.txt        where the server's program is (written by the server)
//! ├── Server/config.json        the server's settings (only read here: port, set up)
//! └── Manager/
//!     ├── logs/                 manager.YYYY-MM-DD.log
//!     └── WebView/              browser data of the Manager's window (last server, mode…)
//! ```
//!
//! `OPENPRES_DATA_DIR` replaces `<data>/OpenPres` (tests, several copies on one computer).
//! Versions before the Manager had its own program used the same `Manager/` folder, so the
//! last server, dashboards and settings carry over (Windows and Linux; macOS keeps browser
//! data per app).

use directories::BaseDirs;
use std::path::PathBuf;

/// `<data>/OpenPres`.
pub fn root() -> PathBuf {
    std::env::var_os("OPENPRES_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| {
        BaseDirs::new()
            .map(|d| d.data_dir().join("OpenPres")) // e.g. AppData\Roaming\OpenPres
            .unwrap_or_else(|| PathBuf::from("./data"))
    })
}

pub fn manager_dir() -> PathBuf {
    root().join("Manager")
}

pub fn logs_dir() -> PathBuf {
    manager_dir().join("logs")
}

/// Browser data of the Manager's window (Windows and Linux; macOS keeps its own).
pub fn webview_dir() -> PathBuf {
    manager_dir().join("WebView")
}

/// The server's settings file: the old place (before 0.4) until the server has moved it.
pub fn server_config_file() -> PathBuf {
    let new = root().join("Server").join("config.json");
    let old = root().join("config.json");
    if !new.exists() && old.exists() {
        old
    } else {
        new
    }
}
