//! Temporary in-app debug log: a small in-memory ring plus an append-only file
//! next to the executable. It is meant to be removed once the current issues
//! are diagnosed.
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const MAX_LINES: usize = 2000;

static LINES: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
static FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
static PATH: OnceLock<PathBuf> = OnceLock::new();

fn lines() -> &'static Mutex<Vec<String>> {
    LINES.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn init() {
    START.get_or_init(Instant::now);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let mut path = exe_dir
        .as_ref()
        .map(|d| d.join("DVIDM-debug.log"))
        .unwrap_or_else(|| std::env::temp_dir().join("DVIDM-debug.log"));
    let mut file = OpenOptions::new().create(true).append(true).open(&path).ok();
    if file.is_none() {
        path = std::env::temp_dir().join("DVIDM-debug.log");
        file = OpenOptions::new().create(true).append(true).open(&path).ok();
    }
    let _ = PATH.set(path);
    let _ = FILE.set(Mutex::new(file));
    write(format!("=== DVIDM {} started ===", env!("CARGO_PKG_VERSION")));
}

pub(crate) fn write(message: impl AsRef<str>) {
    let elapsed = START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    let line = format!("[{elapsed:9.3}] {}", message.as_ref());
    if let Ok(mut buffer) = lines().lock() {
        buffer.push(line.clone());
        let len = buffer.len();
        if len > MAX_LINES {
            buffer.drain(0..len - MAX_LINES);
        }
    }
    if let Some(lock) = FILE.get() {
        if let Ok(mut slot) = lock.lock() {
            if let Some(file) = slot.as_mut() {
                let _ = writeln!(file, "{line}");
                let _ = file.flush();
            }
        }
    }
}

pub(crate) fn snapshot() -> Vec<String> {
    lines()
        .lock()
        .map(|buffer| buffer.clone())
        .unwrap_or_default()
}

pub(crate) fn path() -> PathBuf {
    PATH.get()
        .cloned()
        .unwrap_or_else(|| std::env::temp_dir().join("DVIDM-debug.log"))
}
