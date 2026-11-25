//! The Manager's own updates, from this repository's GitHub Releases (tauri-plugin-updater;
//! endpoint and key in tauri.conf.json). The server list asks when the Manager opens
//! (Update / Remind me later); updating closes it and opens the new version.
//!
//! The beta program is the server's choice (its admin turns it on in Settings → General):
//! the Manager follows the one it managed last, asking its `/api/health`. Betas are carried
//! as X.Y.(Z+1)-A and shown as "X.Y.Z.A beta" (docs/RELEASING.md).
//!
//! Test builds (debug) don't update unless `OPENPRES_UPDATES=1`.

use crate::logging;
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{ipc::Channel, AppHandle};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Where betas (and the stable releases after them) are announced: a fixed release whose
/// latest.json every release replaces.
pub const BETA_FEED: &str = "https://github.com/NathanDSanta/OpenPres-M/releases/download/beta-feed/latest.json";

/// Installed releases update themselves; test builds would "update" to the last release.
pub fn enabled() -> bool {
    !cfg!(debug_assertions) || std::env::var_os("OPENPRES_UPDATES").is_some()
}

/// A version as people see it: "0.7.1-3" is the 3rd beta since 0.7.0, "0.7.0.3 beta".
pub fn display_version(version: &str) -> String {
    if let Some((base, beta)) = version.split_once('-') {
        let parts: Vec<u64> = base.split('.').filter_map(|p| p.parse().ok()).collect();
        if let (Ok(beta), [x, y, z]) = (beta.parse::<u64>(), parts.as_slice()) {
            if *z >= 1 {
                return format!("{x}.{y}.{}.{beta} beta", z - 1);
            }
        }
    }
    version.to_string()
}

/// Whether the server at `url` (e.g. `http://192.168.1.20:8787`) follows the beta program.
/// Unreachable or older servers: no.
async fn server_follows_betas(url: &str) -> bool {
    let ask = async {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok()?
            .get(format!("{}/api/health", url.trim_end_matches('/')))
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?
            .get("beta_program")?
            .as_bool()
    };
    ask.await.unwrap_or(false)
}

fn updater(app: &AppHandle, beta: bool) -> Result<tauri_plugin_updater::Updater, String> {
    let mut builder = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        // Windows: the installer is started and this program ends right away.
        .on_before_exit(|| {
            tracing::info!("closing for the installer");
            logging::flush();
        });
    if beta {
        let url = BETA_FEED.parse().map_err(|e| format!("beta feed: {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    // Test builds can try a local latest.json (still signed with the release key).
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("OPENPRES_UPDATE_ENDPOINT") {
        let url = url.parse().map_err(|e| format!("OPENPRES_UPDATE_ENDPOINT: {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    builder.build().map_err(|e| e.to_string())
}

/// The update found when the Manager opened, kept for "Update".
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize)]
pub struct UpdateInfo {
    version: String,
    /// As people see it ("0.7.0.3 beta").
    name: String,
    current: String,
    current_name: String,
    beta: bool,
    notes: String,
    date: Option<String>,
}

/// Is there a newer version? `server`: the one managed last (its beta program decides
/// whether betas are offered).
#[tauri::command]
pub async fn check_update(app: AppHandle, pending: tauri::State<'_, Pending>, server: Option<String>) -> Result<Option<UpdateInfo>, String> {
    if !enabled() {
        return Ok(None);
    }
    let beta = match &server {
        Some(url) => server_follows_betas(url).await,
        None => false,
    };
    let found = updater(&app, beta)?.check().await.map_err(|e| e.to_string());
    match &found {
        Ok(Some(u)) => tracing::info!("update available: {} -> {}{}", u.current_version, u.version, if beta { " (beta program)" } else { "" }),
        Ok(None) => tracing::info!("no update (running {})", env!("CARGO_PKG_VERSION")),
        Err(e) => tracing::warn!("update check failed: {e}"),
    }
    let update = found?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        name: display_version(&u.version),
        current: u.current_version.clone(),
        current_name: display_version(&u.current_version),
        beta: u.version.contains('-'),
        notes: u.body.clone().unwrap_or_default(),
        date: u.date.map(|d| d.date().to_string()),
    });
    *pending.0.lock().unwrap() = update;
    Ok(info)
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Progress {
    Downloading { done: u64, total: Option<u64> },
    Installing,
}

/// Downloads and installs the update found by `check_update`, then opens the new version.
#[tauri::command]
pub async fn install_update(app: AppHandle, pending: tauri::State<'_, Pending>, on_progress: Channel<Progress>) -> Result<(), String> {
    // Kept (not taken): "Try again" after a failed download uses it again.
    let update = pending.0.lock().unwrap().clone().ok_or("check for updates first")?;
    tracing::info!("updating the Manager to {}", update.version);
    let mut done = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                done += chunk as u64;
                let _ = on_progress.send(Progress::Downloading { done, total });
            },
            || {},
        )
        .await
        .map_err(|e| {
            tracing::error!("update download failed: {e}");
            e.to_string()
        })?;
    let _ = on_progress.send(Progress::Installing);
    // Windows: the installer takes over and this program ends here.
    update.install(bytes).map_err(|e| {
        tracing::error!("update install failed: {e}");
        e.to_string()
    })?;
    tracing::info!("updated; opening the new version");
    logging::flush();
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn betas_are_named_after_the_stable_they_build_on() {
        assert_eq!(display_version("0.7.1-3"), "0.7.0.3 beta");
        assert_eq!(display_version("1.0.1-1"), "1.0.0.1 beta");
        assert_eq!(display_version("0.7.0"), "0.7.0");
        assert_eq!(display_version("0.7.0-rc"), "0.7.0-rc", "not one of ours: as it is");
    }
}
