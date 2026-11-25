//! Log files: `Manager/logs/manager.YYYY-MM-DD.log` (see [`crate::paths`]). A new file every
//! day; the last 14 days are kept.
//!
//! What goes in: everything the program reports (from `debug` up), the pages' console
//! (errors, warnings, logs; also of a server's interface opened in the window) and crashes
//! (panics, with where they happened).
//!
//! `OPENPRES_LOG` changes what is written to the file, `RUST_LOG` what is printed to the
//! terminal; both use the `tracing` filter syntax, e.g. `OPENPRES_LOG=trace`.

use std::{path::Path, sync::Mutex};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Days of logs kept.
pub const KEEP_DAYS: usize = 14;

const NAME: &str = "manager";

/// What the file gets by default: all of the Manager, the libraries' notices.
const FILE_FILTER: &str = "debug,tao=info,wry=info,tauri=info,tauri_runtime_wry=info,hyper=info,hyper_util=info,h2=info,\
reqwest=info,rustls=info,mdns_sd=info";

/// Writes queued lines to the file when dropped (see [`flush`]).
static GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Starts logging: to the Manager's log folder and to the terminal. Call once, first thing
/// in `main`.
pub fn init() {
    let dir = crate::paths::logs_dir();
    let file = match open(&dir) {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
                // Never drop a line (the queue holds 128k; it only fills if the disk stalls).
                .lossy(false)
                .thread_name("log-writer")
                .finish(appender);
            *GUARD.lock().unwrap() = Some(guard);
            let filter = EnvFilter::try_from_env("OPENPRES_LOG").unwrap_or_else(|_| EnvFilter::new(FILE_FILTER));
            Some(
                fmt::layer()
                    .with_writer(writer)
                    .with_ansi(false)
                    .with_thread_names(true)
                    .with_target(true)
                    .with_filter(filter),
            )
        }
        Err(e) => {
            eprintln!("OpenPres Manager: no log file in {}: {e}", dir.display());
            None
        }
    };
    let terminal = fmt::layer()
        .with_writer(std::io::stderr)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")));
    // Also takes in the `log` crate's records (Tauri's own messages).
    let _ = tracing_subscriber::registry().with(file).with(terminal).try_init();
    install_panic_hook();
    tracing::info!(
        "OpenPres Manager {} starting · {} {} · pid {} · args {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::process::id(),
        std::env::args().skip(1).collect::<Vec<_>>(),
    );
    tracing::info!("folder: {} · logs: {}", crate::paths::manager_dir().display(), dir.display());
}

fn open(dir: &Path) -> Result<rolling::RollingFileAppender, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    rolling::Builder::new()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(NAME)
        .filename_suffix("log")
        .max_log_files(KEEP_DAYS)
        .build(dir)
        .map_err(|e| e.to_string())
}

/// Writes what is still queued. Call when the program ends: Tauri ends the process
/// without returning from `run`, so nothing would be dropped otherwise.
pub fn flush() {
    if let Some(guard) = GUARD.lock().ok().and_then(|mut g| g.take()) {
        tracing::info!("stopped");
        drop(guard);
    }
}

/// Crashes go to the log file too (then to the usual place, the terminal).
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread = thread.name().unwrap_or("unnamed");
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".into());
        let location = info.location().map(|l| l.to_string()).unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(target: "panic", "thread '{thread}' panicked at {location}: {message}\n{backtrace}");
        // The main thread panicking ends the program: write the file now.
        if thread == "main" {
            flush();
        }
        previous(info);
    }));
}

/// A line from a page's console (see `pageLog.ts`): written with the window and the page.
pub fn page_line(window: &str, page: &str, level: &str, message: &str) {
    // A page can't flood the file.
    const MAX: usize = 8 * 1024;
    let message = if message.len() > MAX {
        let mut end = MAX;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}… ({} bytes)", &message[..end], message.len())
    } else {
        message.to_string()
    };
    match level {
        "error" => tracing::error!(target: "page", "[{window} {page}] {message}"),
        "warn" => tracing::warn!(target: "page", "[{window} {page}] {message}"),
        "info" => tracing::info!(target: "page", "[{window} {page}] {message}"),
        _ => tracing::debug!(target: "page", "[{window} {page}] {message}"),
    }
}

/// Lets through at most `per_second` page lines a second (a page stuck in a loop).
pub struct PageLimiter {
    window: Mutex<(u64, u32, u32)>, // (second, lines this second, dropped)
    per_second: u32,
}

impl PageLimiter {
    pub const fn new(per_second: u32) -> Self {
        Self {
            window: Mutex::new((0, 0, 0)),
            per_second,
        }
    }

    /// True if the line may be written. Reports how many were dropped once a second passes.
    pub fn allow(&self, now_s: u64) -> bool {
        let mut w = self.window.lock().unwrap();
        if w.0 != now_s {
            if w.2 > 0 {
                tracing::warn!(target: "page", "{} console lines dropped (too many)", w.2);
            }
            *w = (now_s, 0, 0);
        }
        if w.1 < self.per_second {
            w.1 += 1;
            true
        } else {
            w.2 += 1;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_lines_are_limited_per_second() {
        let limiter = PageLimiter::new(3);
        assert_eq!((0..5).filter(|_| limiter.allow(10)).count(), 3);
        assert!(limiter.allow(11), "a new second lets lines through again");
    }

    #[test]
    fn rolling_files_are_named_by_day() {
        let dir = tempfile::tempdir().unwrap();
        let mut appender = open(dir.path()).unwrap();
        use std::io::Write;
        writeln!(appender, "hello").unwrap();
        appender.flush().unwrap();
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(names[0].starts_with("manager.") && names[0].ends_with(".log"), "{names:?}");
    }
}
