//! Win32 window helpers shared by the user interfaces.
//!
//! Also holds the single-instance guard, so a second launch exits quietly
//! instead of creating a second tray icon and a second download engine.
//!
//! Nothing here renders: it only changes the native window's show state, which
//! is what makes "hide to tray" different from "minimize".
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};

#[repr(C)]
struct WinSystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[repr(C)]
struct FileTime {
    low: u32,
    high: u32,
}

/// Format an epoch-millisecond timestamp in local time as `YYYY-MM-DD HH:MM`.
pub(crate) fn format_epoch_ms(ms: u64) -> String {
    const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;
    let ticks = ms.saturating_mul(10_000).saturating_add(EPOCH_DIFF_100NS);
    let file_time = FileTime {
        low: ticks as u32,
        high: (ticks >> 32) as u32,
    };
    let mut local = FileTime { low: 0, high: 0 };
    let mut st = WinSystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe {
        if FileTimeToLocalFileTime(&file_time, &mut local) == 0
            || FileTimeToSystemTime(&local, &mut st) == 0
        {
            return "—".to_string();
        }
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        st.year, st.month, st.day, st.hour, st.minute
    )
}

/// Minutes since local midnight, for list schedule windows.
pub(crate) fn local_minutes() -> u32 {
    unsafe {
        let mut st = WinSystemTime {
            year: 0,
            month: 0,
            day_of_week: 0,
            day: 0,
            hour: 0,
            minute: 0,
            second: 0,
            milliseconds: 0,
        };
        GetLocalTime(&mut st);
        st.hour as u32 * 60 + st.minute as u32
    }
}

/// The window's native handle, stored once the platform created it.
#[derive(Clone, Default)]
pub(crate) struct HwndSlot(std::sync::Arc<AtomicIsize>);

impl HwndSlot {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set(&self, hwnd: isize) {
        self.0.store(hwnd, Ordering::SeqCst);
    }

    pub(crate) fn get(&self) -> isize {
        self.0.load(Ordering::SeqCst)
    }
}

/// Actually hide the window: `SW_HIDE` removes it from the taskbar and from
/// Alt+Tab while the process keeps running. This is not a minimize.
pub(crate) fn hide(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    unsafe {
        ShowWindow(hwnd as *mut c_void, SW_HIDE);
    }
}

/// Show, un-minimize and activate the window (tray click / second instance).
pub(crate) fn restore(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let h = hwnd as *mut c_void;
    unsafe {
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        ShowWindow(h, SW_SHOW);
        SetWindowPos(
            h,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        SetWindowPos(
            h,
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        SetForegroundWindow(h);
    }
}

/// Bytes still free on the volume that holds `path`, and the volume's total
/// size. `None` when the platform cannot answer.
pub(crate) fn free_space(path: &std::path::Path) -> Option<(u64, u64)> {
    use std::os::windows::ffi::OsStrExt as _;
    let mut dir: Vec<u16> = path.as_os_str().encode_wide().collect();
    dir.push(0);
    let mut available = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(dir.as_ptr(), &mut available, &mut total, &mut free) };
    (ok != 0 && total > 0).then_some((available, total))
}

/// Visible but minimized to the taskbar.
pub(crate) fn is_minimized(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { IsIconic(hwnd as *mut c_void) != 0 }
}

/// Hidden from the taskbar entirely (as opposed to minimized).
pub(crate) fn is_visible(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { IsWindowVisible(hwnd as *mut c_void) != 0 }
}

/// Keeps a single running instance. The mutex lives for the process lifetime.
pub(crate) struct InstanceGuard(*mut c_void);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// `None` when another instance already holds the mutex.
pub(crate) fn acquire_single_instance() -> Option<InstanceGuard> {
    const ERROR_ALREADY_EXISTS: u32 = 183;
    let name: Vec<u16> = "Local\\DVIDM_SingleInstance\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            // Could not create the mutex; prefer starting over blocking.
            return Some(InstanceGuard(handle));
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            None
        } else {
            Some(InstanceGuard(handle))
        }
    }
}

// --- Start with Windows (per-user Run key) -----------------------------

const AUTOSTART_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const AUTOSTART_VALUE: &str = "DVIDM";
const HKEY_CURRENT_USER: isize = 0x80000001u32 as i32 as isize;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_SZ: u32 = 1;
const ERROR_FILE_NOT_FOUND: i32 = 2;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn autostart_enabled() -> bool {
    let key = wide(AUTOSTART_KEY);
    let name = wide(AUTOSTART_VALUE);
    let mut hkey: *mut c_void = std::ptr::null_mut();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER as *mut c_void,
            key.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        ) != 0
        {
            return false;
        }
        let mut ty = 0u32;
        let mut bytes = 0u32;
        let exists = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            std::ptr::null_mut(),
            &mut bytes,
        ) == 0;
        RegCloseKey(hkey);
        exists
    }
}

pub(crate) fn set_autostart(enabled: bool) -> Result<(), String> {
    let key = wide(AUTOSTART_KEY);
    let name = wide(AUTOSTART_VALUE);
    let mut hkey: *mut c_void = std::ptr::null_mut();
    unsafe {
        let rc = RegOpenKeyExW(
            HKEY_CURRENT_USER as *mut c_void,
            key.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if rc != 0 {
            return Err(format!("Could not open the startup registry key (error {rc})"));
        }
        let rc = if enabled {
            let exe = std::env::current_exe()
                .map_err(|e| format!("Could not find the application path: {e}"))?;
            let value = wide(&format!("\"{}\"", exe.display()));
            RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                REG_SZ,
                value.as_ptr() as *const u8,
                (value.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(hkey, name.as_ptr())
        };
        RegCloseKey(hkey);
        if rc != 0 && !(!enabled && rc == ERROR_FILE_NOT_FOUND) {
            return Err(format!("Could not update startup settings (error {rc})"));
        }
        Ok(())
    }
}

/// Keep the Run entry pointing at this executable after a move or update.
pub(crate) fn refresh_autostart() {
    if autostart_enabled() {
        let _ = set_autostart(true);
    }
}

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        hkey: *mut c_void,
        sub_key: *const u16,
        options: u32,
        sam: u32,
        result: *mut *mut c_void,
    ) -> i32;
    fn RegSetValueExW(
        hkey: *mut c_void,
        name: *const u16,
        reserved: u32,
        ty: u32,
        data: *const u8,
        bytes: u32,
    ) -> i32;
    fn RegDeleteValueW(hkey: *mut c_void, name: *const u16) -> i32;
    fn RegQueryValueExW(
        hkey: *mut c_void,
        name: *const u16,
        reserved: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        bytes: *mut u32,
    ) -> i32;
    fn RegCloseKey(hkey: *mut c_void) -> i32;
}

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const SW_RESTORE: i32 = 9;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_SHOWWINDOW: u32 = 0x0040;
const HWND_TOPMOST: *mut c_void = -1isize as *mut c_void;
const HWND_NOTOPMOST: *mut c_void = -2isize as *mut c_void;

#[link(name = "user32")]
extern "system" {
    fn ShowWindow(hwnd: *mut c_void, cmd: i32) -> i32;
    fn IsWindowVisible(hwnd: *mut c_void) -> i32;
    fn IsIconic(hwnd: *mut c_void) -> i32;
    fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    fn SetWindowPos(
        hwnd: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(lp: *mut WinSystemTime);
    fn FileTimeToLocalFileTime(
        file_time: *const FileTime,
        local_file_time: *mut FileTime,
    ) -> i32;
    fn FileTimeToSystemTime(file_time: *const FileTime, system_time: *mut WinSystemTime) -> i32;
    fn CreateMutexW(
        attributes: *const c_void,
        initial_owner: i32,
        name: *const u16,
    ) -> *mut c_void;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
    fn GetDiskFreeSpaceExW(
        dir: *const u16,
        available: *mut u64,
        total: *mut u64,
        free: *mut u64,
    ) -> i32;
}
