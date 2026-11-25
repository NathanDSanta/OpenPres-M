#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! OpenPres Manager: finds OpenPres servers on the network (or starts the one on this
//! computer) and opens the chosen server's interface in its window; everything is done
//! through it. The server is a program of its own (the OpenPres repository).

mod discovery;
mod local;
mod logging;
mod paths;
mod updater;

use serde::Deserialize;
use tauri::{Manager, RunEvent, WebviewWindowBuilder, WindowEvent};

const MAIN_WINDOW: &str = "main";

fn main() {
    logging::init();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Discovery(discovery::Browser::start()))
        .manage(updater::Pending::default())
        .setup(|app| {
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == MAIN_WINDOW)
                .cloned()
                .ok_or("no main window in tauri.conf.json")?;
            let window = WebviewWindowBuilder::from_config(app.handle(), &config)?;
            // Its browser data (last server, mode, dashboards) in OpenPres/Manager/WebView, where
            // the Manager kept it before it was a program of its own. macOS keeps it per app.
            #[cfg(any(windows, target_os = "linux"))]
            let window = window.data_directory(paths::webview_dir());
            with_title_bar(window)
                // Links that open a new window (help pages, the YouTube sign-in, downloads) go
                // to the computer's browser.
                .on_new_window(|url, _| {
                    if let Err(e) = open_url(url.as_str()) {
                        tracing::warn!("could not open {url}: {e}");
                    }
                    tauri::webview::NewWindowResponse::Deny
                })
                .build()?;
            // It opens maximized ("maximized" in tauri.conf.json isn't kept on Windows, where
            // the window opened at its restored size).
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let _ = window.maximize();
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == MAIN_WINDOW && matches!(event, WindowEvent::CloseRequested { .. }) {
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            discovered_servers,
            local_server_status,
            start_local_server,
            set_this_computer,
            page_log,
            updater::check_update,
            updater::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Manager");
    app.run(|_, event| {
        if let RunEvent::Exit = event {
            logging::flush();
        }
    });
}

/// No system title bar: the window buttons (`titlebar.js`, run on every page the window
/// loads) sit in OpenPres's own top bar, float in the corner of pages without one, or (an
/// older server's pages) get a slim bar. On macOS the system keeps its traffic lights, over
/// the page.
fn with_title_bar<'a, R: tauri::Runtime, M: Manager<R>>(builder: WebviewWindowBuilder<'a, R, M>) -> WebviewWindowBuilder<'a, R, M> {
    let builder = builder.initialization_script(include_str!("titlebar.js"));
    #[cfg(target_os = "macos")]
    {
        builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Windows keeps the shadow (and the rounded corners on Windows 11) of a framed window.
        builder.decorations(false).shadow(true)
    }
}

/// Opens a web address in the computer's browser.
fn open_url(url: &str) -> Result<(), String> {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.args(["url.dll,FileProtocolHandler", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // no console window
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ---------- commands ----------

/// Servers found on the network (mDNS).
struct Discovery(discovery::Browser);

#[tauri::command]
fn discovered_servers(discovery: tauri::State<'_, Discovery>) -> Vec<discovery::FoundServer> {
    discovery.0.list()
}

#[tauri::command]
async fn local_server_status() -> local::LocalStatus {
    local::status().await
}

/// Starts the server on this computer, in the background.
#[tauri::command]
fn start_local_server() -> Result<(), String> {
    local::start_server()
}

/// Uses this computer as a server, or not.
#[tauri::command]
async fn set_this_computer(server: bool) -> Result<(), String> {
    local::set_this_computer(server).await
}

#[derive(Deserialize)]
struct PageLine {
    level: String,
    message: String,
}

static PAGE_LIMIT: logging::PageLimiter = logging::PageLimiter::new(200);

/// A page's console lines, for the log file (see `src/lib/pageLog.ts`). Also allowed for the
/// server's interface opened in the window (capabilities/manager-pages.json).
#[tauri::command]
fn page_log(webview: tauri::Webview, lines: Vec<PageLine>) {
    // Where the lines come from, without the query (it can hold a key).
    let page = webview
        .url()
        .map(|u| {
            let web = u.scheme().starts_with("http") && u.host_str().is_some_and(|h| h != "tauri.localhost");
            match (web, u.host_str(), u.port()) {
                (true, Some(host), Some(port)) => format!("{host}:{port}{}", u.path()),
                (true, Some(host), None) => format!("{host}{}", u.path()),
                _ => u.path().to_string(),
            }
        })
        .unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    for line in lines {
        if PAGE_LIMIT.allow(now) {
            logging::page_line(webview.label(), &page, &line.level, &line.message);
        }
    }
}
