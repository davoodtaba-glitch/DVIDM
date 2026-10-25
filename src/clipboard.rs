use crate::engine::guess_name;
use std::ffi::c_void;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

const CF_UNICODETEXT: u32 = 13;

const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hwnd: *mut c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
    fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalFree(handle: *mut c_void) -> *mut c_void;
    fn GlobalLock(handle: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(handle: *mut c_void) -> i32;
    fn GlobalSize(handle: *mut c_void) -> usize;
}

/// Put Unicode text on the Windows clipboard.
pub(crate) fn write_text(text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !opened {
            return false;
        }
        let _ = EmptyClipboard();
        let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if mem.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        let ptr = GlobalLock(mem);
        if ptr.is_null() {
            let _ = GlobalFree(mem);
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
        let _ = GlobalUnlock(mem);
        // On success the clipboard owns the block and must not be freed here.
        let ok = !SetClipboardData(CF_UNICODETEXT, mem).is_null();
        if !ok {
            let _ = GlobalFree(mem);
        }
        let _ = CloseClipboard();
        ok
    }
}

/// Read Unicode text from the Windows clipboard.
pub(crate) fn read_text() -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return None;
        }
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        if !opened {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        let mut text = None;
        if !handle.is_null() {
            let ptr = GlobalLock(handle);
            if !ptr.is_null() {
                let units = GlobalSize(handle) / 2;
                let slice = std::slice::from_raw_parts(ptr as *const u16, units);
                let end = slice.iter().position(|&c| c == 0).unwrap_or(units);
                text = Some(String::from_utf16_lossy(&slice[..end]));
                let _ = GlobalUnlock(handle);
            }
        }
        let _ = CloseClipboard();
        text
    }
}

/// Pick unique http(s) links out of arbitrary clipboard text.
pub(crate) fn extract_urls(text: &str) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(c, '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';')
        });
        if (token.starts_with("http://") || token.starts_with("https://"))
            && !urls.iter().any(|u| u == token)
        {
            urls.push(token.to_string());
        }
    }
    urls
}

pub(crate) struct ClipItem {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) size: Option<u64>,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) selected: bool,
}

pub(crate) struct ClipImport {
    pub(crate) items: Vec<ClipItem>,
    // Destination-folder popup state, only drawn by the egui front end.
    #[allow(dead_code)]
    pub(crate) dest: Option<String>,
    #[allow(dead_code)]
    pub(crate) dest_open: bool,
    #[allow(dead_code)]
    pub(crate) dest_anchor: (f32, f32),
    #[allow(dead_code)]
    pub(crate) dest_opened_at: u64,
    /// Header suffix: "clipboard" or "Chrome".
    pub(crate) source: &'static str,
    tx: Sender<(usize, Result<Option<u64>, String>)>,
    rx: Receiver<(usize, Result<Option<u64>, String>)>,
}

impl ClipImport {
    pub(crate) fn new(urls: Vec<String>, source: &'static str) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut clip = Self {
            items: Vec::new(),
            dest: None,
            dest_open: false,
            dest_anchor: (0.0, 0.0),
            dest_opened_at: 0,
            source,
            tx,
            rx,
        };
        clip.push_urls(urls);
        clip
    }

    /// Add more links to an open import window (e.g. another Chrome send).
    pub(crate) fn push_urls(&mut self, urls: Vec<String>) {
        for url in urls {
            if self.items.iter().any(|item| item.url == url) {
                continue;
            }
            let i = self.items.len();
            let tx = self.tx.clone();
            let probe_url = url.clone();
            thread::spawn(move || {
                let result = crate::engine::probe_size(&probe_url);
                let _ = tx.send((i, result));
            });
            self.items.push(ClipItem {
                name: guess_name(&url),
                url,
                size: None,
                loading: true,
                error: None,
                selected: true,
            });
        }
    }

    /// Drain size probes that finished since the last frame.
    pub(crate) fn poll(&mut self) {
        while let Ok((i, result)) = self.rx.try_recv() {
            if let Some(item) = self.items.get_mut(i) {
                item.loading = false;
                match result {
                    Ok(size) => item.size = size,
                    Err(e) => item.error = Some(e),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extract_urls;

    #[test]
    fn extracts_unique_links() {
        let text = "https://a.test/x.zip, \"https://b.test/y.zip\"\n\
                    ftp://ignored https://a.test/x.zip https://c.test/z.zip?x=1";
        let urls = extract_urls(text);
        assert_eq!(urls.len(), 3);
        assert_eq!(urls[0], "https://a.test/x.zip");
        assert_eq!(urls[1], "https://b.test/y.zip");
        assert_eq!(urls[2], "https://c.test/z.zip?x=1");
    }

    #[test]
    fn ignores_text_without_links() {
        assert!(extract_urls("no links here").is_empty());
    }
}
