//! The contract between a user interface and the download engine.
//!
//! A UI never touches [`Manager`] directly: it sends [`Command`]s and reacts to
//! the [`Event`]s they produce. Time-driven work (speed sampling, list caps,
//! schedule windows, shutdown countdowns) happens in [`DownloadService::tick`],
//! which is independent of any window being visible.
use crate::engine::Snapshot;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Add {
        url: String,
        dir: PathBuf,
        file_name: Option<String>,
        /// Download list the file belongs to; None means the default list.
        list: Option<String>,
        /// Join the auto-starting queue instead of waiting for a manual start.
        queued: bool,
    },
    Pause(Vec<String>),
    Resume(Vec<String>),
    PauseAll,
    ResumeAll,
    Remove {
        ids: Vec<String>,
        delete_files: bool,
    },
    /// Move the given downloads one slot up or down in the queue.
    MoveSelected {
        ids: Vec<String>,
        up: bool,
    },
    SetConnections(u32),
    SetLimitKbps(u64),
    SetRetry {
        parts: bool,
        after_secs: u32,
    },
    SetSaveDir(PathBuf),
    /// Folder for partial files, persisted for the next run.
    SetTempDir(PathBuf),
    SetQueued(bool),
    /// Light or dark interface, persisted for the next run.
    SetTheme(bool),
    CreateList(String),
    RenameList {
        from: String,
        to: String,
    },
    DeleteList(String),
    AssignList {
        ids: Vec<String>,
        name: String,
    },
    SetListOptions {
        name: String,
        max_parallel: u32,
        /// Local-time window in minutes since midnight.
        sched: Option<(u32, u32)>,
        shutdown_at_end: bool,
    },
    /// Label colour index for a list.
    SetListColor {
        name: String,
        color: u8,
    },
    /// Custom label colour for a list (#rrggbb); `None` returns to the palette.
    SetListColorHex {
        name: String,
        hex: Option<String>,
    },
    /// Persisted width of one download-table column.
    SetColumnWidth {
        key: String,
        width: u32,
    },
    /// Show or hide one optional download-table column.
    SetColumnHidden {
        key: String,
        hidden: bool,
    },
    /// Drop every custom column width and show all columns again.
    ResetColumns,
    /// Interface font family, persisted for the next run.
    SetFont(String),
    /// Navigation sidebar width in logical pixels, persisted for the next run.
    SetSidebarWidth(u32),
    /// Show or silence the download-complete desktop notification.
    SetNotifyComplete(bool),
    /// Scroll the download list to a newly added download.
    SetFocusNewDownload(bool),
    /// Remember whether the floating downloads card is open.
    SetFloatingCard(bool),
    /// Persist the table sort column (empty = unsorted) and direction.
    SetSort { col: String, desc: bool },
    /// Remember that the user chose the startup-with-Windows option.
    AutostartConfigured,
    /// Timestamp of the last update check, persisted for the 2-day schedule.
    SetLastUpdateCheck(u64),
    /// Newest version the background check has already announced.
    SetUpdateNotified(String),
    /// Browser shortcut modifier: "ctrl", "alt" or "both".
    SetBrowserModifier(String),
    /// Interface language: "system", "en", "fa" or "ar".
    SetUiLang(String),
    SetProxyMode(String),
    /// Manual proxy protocol: "http" or "socks5".
    SetProxyKind(String),
    SetProxyFields {
        host: String,
        port: u16,
        user: String,
        pass: String,
    },
    SetProxyProtocols {
        http: bool,
        https: bool,
        ftp: bool,
    },
    SetProxyFromBrowser(bool),
    /// Extensions the browser Ctrl/Alt+click shortcut captures; the raw
    /// settings text, parsed and normalized by the service.
    SetDirectFormats(String),
    /// Drop a pending machine shutdown started by a list schedule.
    CancelShutdown,
    /// Stop every worker, persist progress and release the engine.
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Added(String),
    Error(String),
    /// A download just reached `Done`; the payload is its file name.
    Finished(String),
    /// A list schedule reached its closing time; the machine should power off.
    ShutdownMachine,
    /// The pending shutdown countdown was set or moved.
    ShutdownAt(Option<u64>),
}

/// Read-only view of what the interface renders. Produced by the service so
/// the UI can render without locking the engine.
#[derive(Clone, Default)]
pub(crate) struct ViewState {
    pub(crate) snaps: Vec<Snapshot>,
}
