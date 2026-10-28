//! Shared application state and the only path from a UI to the engine.
//!
//! The service owns the [`Manager`], the persisted preferences and the queue
//! bookkeeping (auto-queued, user-paused, schedule windows). A UI renders
//! [`DownloadService::view`], sends [`Command`]s, and calls
//! [`DownloadService::tick`] on a timer of its own choosing. Because ticking is
//! independent of any window, downloads and schedules keep running while the
//! window is hidden in the tray.
use super::prefs::{load_prefs, DownloadList, Prefs, LIST_COLOR_COUNT, MAX_LIMIT_KBPS};
use super::proto::{Command, Event, ViewState};
use super::schedule::{list_rules, plan_limit_actions, schedule_events};
use super::sys;
use crate::engine::{now_ms, Category, Manager, Snapshot, Status};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(crate) struct DownloadService {
    mgr: Manager,
    prefs: Prefs,
    /// Tasks the list limiter paused; only these auto-resume when a slot frees.
    auto_queued: HashSet<String>,
    /// Tasks the user paused by hand; the scheduler never starts these.
    user_paused: HashSet<String>,
    /// Tasks the user started by hand; a closed schedule window must not pause these.
    user_started: HashSet<String>,
    /// Last seen open state of each scheduled list, for detecting the opening moment.
    sched_open: HashMap<String, bool>,
    /// Deadline (ms) of a pending forced shutdown requested by a list schedule.
    shutdown_at: Option<u64>,
    /// Downloads already reported as finished, so a completion is announced once.
    known_done: HashSet<String>,
    /// Previous status per download, for the debug log.
    last_status: HashMap<String, Status>,
    /// URL currently pushed into the engine; `None` means direct.
    proxy_current: Option<String>,
    proxy_checked: std::time::Instant,
    view: ViewState,
}

impl DownloadService {
    pub(crate) fn new() -> Self {
        let mut prefs = load_prefs(&crate::engine::default_save_dir());
        let mut reset = false;

        let temp_pref = prefs.temp_dir.trim().to_string();
        let temp = if temp_pref.is_empty() {
            let target = crate::engine::default_temp_dir();
            migrate_temp_files(&crate::engine::legacy_temp_dir(), &target);
            target
        } else {
            PathBuf::from(&temp_pref)
        };
        let temp = if std::fs::create_dir_all(&temp).is_ok() && temp.is_dir() {
            temp
        } else {
            let fallback = crate::engine::default_temp_dir();
            let _ = std::fs::create_dir_all(&fallback);
            if !temp_pref.is_empty() {
                super::log::write(format!(
                    "temp folder unavailable ({temp_pref}), using {fallback:?}"
                ));
                prefs.temp_dir.clear();
                reset = true;
            }
            fallback
        };
        crate::engine::set_temp_dir(&temp);

        // A settings file copied from another PC may point at a drive this
        // machine does not have; fall back to the Windows Downloads folder.
        let dir_pref = prefs.dir.trim().to_string();
        if !dir_pref.is_empty() {
            let dir = PathBuf::from(&dir_pref);
            if std::fs::create_dir_all(&dir).is_err() || !dir.is_dir() {
                let fallback = crate::engine::default_save_dir();
                let _ = std::fs::create_dir_all(&fallback);
                super::log::write(format!(
                    "download folder unavailable ({dir_pref}), using {fallback:?}"
                ));
                prefs.dir.clear();
                reset = true;
            }
        }

        let mgr = Manager::new();
        let mut svc = Self {
            mgr,
            prefs,
            auto_queued: HashSet::new(),
            user_paused: HashSet::new(),
            user_started: HashSet::new(),
            sched_open: HashMap::new(),
            shutdown_at: None,
            known_done: HashSet::new(),
            last_status: HashMap::new(),
            // A sentinel so the first apply always logs and pushes the value.
            proxy_current: Some(String::new()),
            proxy_checked: std::time::Instant::now(),
            view: ViewState::default(),
        };
        svc.apply_settings();
        // Serve the browser-shortcut format list from the local HTTP server.
        super::server::set_direct_formats(svc.prefs.direct_formats.clone());
        super::server::set_browser_modifier(svc.prefs.browser_modifier.clone());
        // Start with Windows unless the user has already made that choice;
        // settings written by older builds have no marker, so they get the
        // new default once.
        if !svc.prefs.autostart_configured {
            match super::sys::set_autostart(true) {
                Ok(()) => super::log::write("autostart enabled by default"),
                Err(err) => super::log::write(format!("autostart default failed: {err}")),
            }
            svc.prefs.autostart_configured = true;
            reset = true;
        }
        if reset
            || normalize_lists(&mut svc.prefs.lists)
            || !super::prefs::settings_have_direct_formats()
        {
            svc.prefs.save();
        }
        svc.view.snaps = svc.mgr.list();
        svc.known_done = svc
            .view
            .snaps
            .iter()
            .filter(|s| s.status == Status::Done)
            .map(|s| s.id.clone())
            .collect();
        svc.last_status = svc
            .view
            .snaps
            .iter()
            .map(|s| (s.id.clone(), s.status))
            .collect();
        super::log::write(format!(
            "startup save={:?} temp={:?} downloads={}",
            svc.save_dir(),
            svc.temp_dir(),
            svc.view.snaps.len()
        ));
        svc
    }

    pub(crate) fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    pub(crate) fn view(&self) -> &ViewState {
        &self.view
    }

    pub(crate) fn lists(&self) -> &[DownloadList] {
        &self.prefs.lists
    }

    pub(crate) fn save_dir(&self) -> PathBuf {
        let dir = self.prefs.dir.trim();
        if dir.is_empty() {
            self.mgr.save_dir.clone()
        } else {
            PathBuf::from(dir)
        }
    }

    /// Folder that holds partial files right now.
    pub(crate) fn temp_dir(&self) -> PathBuf {
        let dir = self.prefs.temp_dir.trim();
        if dir.is_empty() {
            crate::engine::default_temp_dir()
        } else {
            PathBuf::from(dir)
        }
    }

    /// Tasks held back by a list cap, for the "Queued" badge.
    pub(crate) fn auto_queued_ids(&self) -> HashSet<String> {
        self.auto_queued.clone()
    }

    /// Tasks in a list that has a time window, for the "Scheduled" badge.
    pub(crate) fn scheduled_ids(&self) -> HashSet<String> {
        self.prefs
            .lists
            .iter()
            .filter(|l| l.sched_start.is_some() && l.sched_end.is_some())
            .flat_map(|l| l.items.iter().cloned())
            .collect()
    }

    /// An unfinished download with the same URL, if any.
    pub(crate) fn unfinished_duplicate(&self, url: &str) -> Option<Snapshot> {
        self.mgr.unfinished_duplicate(url)
    }

    /// A free destination name for `name` in `dir`.
    pub(crate) fn suggest_name(&self, dir: &Path, name: &str) -> String {
        self.mgr.suggest_name(dir, name)
    }

    /// Change the source address of a paused download, keeping its progress.
    pub(crate) fn set_url(&mut self, id: &str, url: &str) -> Result<(), String> {
        self.mgr.set_url(id, url)?;
        self.view.snaps = self.mgr.list();
        Ok(())
    }

    /// Move one download, and its partial data, into another folder.
    pub(crate) fn set_dest(&mut self, id: &str, dir: PathBuf) -> Result<(), String> {
        self.mgr.set_dest(id, dir)?;
        self.view.snaps = self.mgr.list();
        Ok(())
    }

    /// Push the persisted preferences into the engine (connections, limits,
    /// retry policy, save folder).
    fn apply_settings(&mut self) {
        self.mgr.connections = self.prefs.conns.clamp(1, 32);
        self.mgr.retry_parts.store(
            self.prefs.retry_parts,
            std::sync::atomic::Ordering::SeqCst,
        );
        self.mgr.retry_after.store(
            self.prefs.retry_after as u64,
            std::sync::atomic::Ordering::SeqCst,
        );
        self.mgr
            .limiter
            .set(self.prefs.limit_kbps.min(MAX_LIMIT_KBPS) * 1024);
        let dir = self.save_dir();
        let _ = std::fs::create_dir_all(&dir);
        self.mgr.save_dir = dir;
        self.apply_proxy();
    }

    fn apply_proxy(&mut self) {
        let mode = super::prefs::normalize_proxy_mode(&self.prefs.proxy_mode);
        let mut url: Option<String> = None;
        if mode == "manual" && (self.prefs.proxy_http || self.prefs.proxy_https) {
            let host = self.prefs.proxy_host.trim();
            if host.is_empty() {
                super::log::write("proxy: manual mode but the address is empty");
            } else {
                url = Some(build_proxy_url(
                    host,
                    self.prefs.proxy_port,
                    &self.prefs.proxy_kind,
                    self.prefs.proxy_user.trim(),
                    self.prefs.proxy_pass.trim(),
                ));
            }
        } else if mode == "system" || (mode == "none" && self.prefs.proxy_from_browser) {
            url = super::sys::system_proxy();
            if url.is_none() {
                super::log::write("proxy: system mode but Windows has no proxy configured");
            }
        }
        if url == self.proxy_current {
            return;
        }
        match &url {
            Some(value) => super::log::write(format!("proxy: {}", mask_proxy(value))),
            None => super::log::write("proxy: direct"),
        }
        crate::engine::set_proxy(url.clone());
        self.proxy_current = url;
    }

    /// Time-driven work: speed sampling, list caps, schedules, shutdown.
    pub(crate) fn tick(&mut self) -> Vec<Event> {
        // Windows can enable/disable its proxy while DVIDM is running.
        let mode = super::prefs::normalize_proxy_mode(&self.prefs.proxy_mode);
        if (mode == "system" || (mode == "none" && self.prefs.proxy_from_browser))
            && self.proxy_checked.elapsed() >= std::time::Duration::from_secs(2)
        {
            self.proxy_checked = std::time::Instant::now();
            self.apply_proxy();
        }
        let mut events = Vec::new();
        self.mgr.tick_speeds();
        self.view.snaps = self.mgr.list();
        let snapshots: Vec<(String, String, Status, Option<String>)> = self
            .view
            .snaps
            .iter()
            .map(|s| (s.id.clone(), s.name.clone(), s.status, s.error.clone()))
            .collect();
        let mut present = HashSet::new();
        for (id, name, status, error) in snapshots {
            present.insert(id.clone());
            match self.last_status.insert(id, status) {
                Some(previous) if previous != status => {
                    let suffix = error
                        .map(|e| format!(" error={e}"))
                        .unwrap_or_default();
                    super::log::write(format!(
                        "download {name}: {previous:?} -> {status:?}{suffix}"
                    ));
                }
                None => {
                    let suffix = error
                        .map(|e| format!(" error={e}"))
                        .unwrap_or_default();
                    super::log::write(format!("download {name}: {status:?} (new){suffix}"));
                }
                _ => {}
            }
        }
        self.last_status.retain(|id, _| present.contains(id));
        for snap in &self.view.snaps {
            if snap.status == Status::Done && self.known_done.insert(snap.id.clone()) {
                events.push(Event::Finished(snap.name.clone()));
            }
        }
        self.enforce_lists(&mut events);
        if let Some(at) = self.shutdown_at {
            if now_ms() >= at {
                self.shutdown_at = None;
                events.push(Event::ShutdownMachine);
            }
        }
        events
    }

    pub(crate) fn apply(&mut self, cmd: Command) -> Vec<Event> {
        let mut events = Vec::new();
        super::log::write(format!("command {cmd:?}"));
        match cmd {
            Command::Add {
                url,
                dir,
                file_name,
                list,
                queued,
            } => {
                self.mgr.connections = self.prefs.conns.clamp(1, 32);
                let default = self
                    .prefs
                    .lists
                    .iter()
                    .find(|l| l.is_default)
                    .map(|l| l.name.clone());
                let list_name = list.clone().or_else(|| default.clone());
                let list_folder = list_name
                    .as_deref()
                    .map(crate::engine::sanitize)
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "Unlisted".into());
                let guess = file_name
                    .clone()
                    .unwrap_or_else(|| crate::engine::guess_name(&url));
                let cat_folder = match crate::engine::category_of(&guess) {
                    Category::Compressed => "Archives",
                    Category::Documents => "Documents",
                    Category::Media => "Media",
                    Category::Programs => "Apps",
                    _ => "Other",
                };
                let target_dir = dir.join(&list_folder).join(cat_folder);
                let temp_sub = format!("{list_folder}/{cat_folder}");
                // With the queue off a new download starts at once; the flag
                // only holds it back when the user asked for a queue.
                match self.mgr.add(&url, Some(target_dir), queued, file_name.as_deref(), &temp_sub) {
                    Ok(id) => {
                        if queued {
                            // "Don't start immediately" is a user hold, not a
                            // list-cap pause: the limiter must not auto-resume it.
                            self.user_paused.insert(id.clone());
                        }
                        if let Some(name) = list_name {
                            self.assign_to_list(&[id.clone()], &name);
                        }
                        events.push(Event::Added(id));
                    }
                    Err(e) => events.push(Event::Error(e)),
                }
            }
            Command::Pause(ids) => {
                for id in ids {
                    self.mgr.pause(&id);
                    self.auto_queued.remove(&id);
                    self.user_started.remove(&id);
                    self.user_paused.insert(id);
                }
            }
            Command::Resume(ids) => {
                for id in ids {
                    self.mgr.resume(&id);
                    self.auto_queued.remove(&id);
                    self.user_paused.remove(&id);
                    self.user_started.insert(id);
                }
            }
            Command::PauseAll => {
                self.mgr.pause_all();
                self.auto_queued.clear();
                self.user_started.clear();
                let ids: Vec<String> = self.mgr.list().into_iter().map(|s| s.id).collect();
                self.user_paused.extend(ids);
            }
            Command::ResumeAll => {
                let paused: Vec<String> = self
                    .mgr
                    .list()
                    .into_iter()
                    .filter(|s| matches!(s.status, Status::Paused | Status::Failed))
                    .map(|s| s.id)
                    .collect();
                self.mgr.resume_all();
                self.auto_queued.clear();
                self.user_paused.clear();
                self.user_started.extend(paused);
            }
            Command::Remove { ids, delete_files } => {
                for id in ids {
                    self.mgr.remove(&id, delete_files);
                    self.auto_queued.remove(&id);
                    self.user_paused.remove(&id);
                    self.user_started.remove(&id);
                }
                self.prune_lists();
            }
            Command::MoveSelected { ids, up } => {
                let selected: HashSet<String> = ids.iter().cloned().collect();
                let mut order: Vec<String> = self.mgr.list().into_iter().map(|s| s.id).collect();
                if super::schedule::move_selection(&mut order, &selected, up) {
                    self.mgr.set_order(&order);
                }
            }
            Command::SetConnections(n) => {
                self.prefs.conns = n.clamp(1, 32);
                self.mgr.connections = self.prefs.conns;
                self.prefs.save();
            }
            Command::SetLimitKbps(kbps) => {
                self.prefs.limit_kbps = kbps.min(MAX_LIMIT_KBPS);
                self.mgr.limiter.set(self.prefs.limit_kbps * 1024);
                self.prefs.save();
            }
            Command::SetRetry { parts, after_secs } => {
                self.prefs.retry_parts = parts;
                self.prefs.retry_after = after_secs.clamp(3, 300);
                self.apply_settings();
                self.prefs.save();
            }
            Command::SetSaveDir(dir) => {
                self.prefs.dir = dir.to_string_lossy().into_owned();
                self.apply_settings();
                self.prefs.save();
            }
            Command::SetTempDir(dir) => match self.mgr.set_temp_dir(dir.clone()) {
                Ok(()) => {
                    self.prefs.temp_dir = dir.to_string_lossy().into_owned();
                    self.prefs.save();
                }
                Err(e) => events.push(Event::Error(e)),
            },
            Command::SetQueued(on) => {
                self.prefs.queued = on;
                self.prefs.save();
            }
            Command::SetTheme(dark) => {
                self.prefs.theme_dark = dark;
                self.prefs.save();
            }
            Command::CreateList(name) => {
                let name = name.trim().to_string();
                if !name.is_empty()
                    && !self
                        .prefs
                        .lists
                        .iter()
                        .any(|l| l.name.eq_ignore_ascii_case(&name))
                {
                    self.prefs.lists.push(DownloadList {
                        name,
                        items: Vec::new(),
                        color: (self.prefs.lists.len() % LIST_COLOR_COUNT) as u8,
                        color_hex: None,
                        max_parallel: 0,
                        sched_start: None,
                        sched_end: None,
                        shutdown_at_end: false,
                        is_default: false,
                    });
                    self.prefs.save();
                }
            }
            Command::RenameList { from, to } => {
                let to = to.trim().to_string();
                let taken = self
                    .prefs
                    .lists
                    .iter()
                    .any(|l| l.name != from && l.name.eq_ignore_ascii_case(&to));
                if !to.is_empty() && !taken {
                    if let Some(l) = self.prefs.lists.iter_mut().find(|l| l.name == from) {
                        l.name = to.clone();
                    }
                    if let Some(open) = self.sched_open.remove(&from) {
                        self.sched_open.insert(to, open);
                    }
                    self.prefs.save();
                }
            }
            Command::DeleteList(name) => {
                if let Some(i) = self.prefs.lists.iter().position(|l| l.name == name) {
                    if !self.prefs.lists[i].is_default {
                        self.prefs.lists.remove(i);
                        self.prefs.save();
                    }
                }
            }
            Command::AssignList { ids, name } => {
                self.assign_to_list(&ids, &name);
            }
            Command::SetListOptions {
                name,
                max_parallel,
                sched,
                shutdown_at_end,
            } => {
                if let Some(i) = self.prefs.lists.iter().position(|l| l.name == name) {
                    let had_sched = self.prefs.lists[i].sched_start.is_some()
                        && self.prefs.lists[i].sched_end.is_some();
                    let members = self.prefs.lists[i].items.clone();
                    let l = &mut self.prefs.lists[i];
                    l.max_parallel = max_parallel;
                    l.sched_start = sched.map(|s| s.0);
                    l.sched_end = sched.map(|s| s.1);
                    l.shutdown_at_end = shutdown_at_end;
                    self.prefs.save();
                    // Turning the window off must release what it held back.
                    if had_sched && sched.is_none() {
                        for id in members {
                            if self.auto_queued.remove(&id) {
                                self.mgr.resume(&id);
                            }
                        }
                    }
                }
            }
            Command::SetListColor { name, color } => {
                if let Some(l) = self.prefs.lists.iter_mut().find(|l| l.name == name) {
                    l.color = color % (LIST_COLOR_COUNT as u8);
                    l.color_hex = None;
                    self.prefs.save();
                }
            }
            Command::SetListColorHex { name, hex } => {
                if let Some(l) = self.prefs.lists.iter_mut().find(|l| l.name == name) {
                    l.color_hex = hex.as_deref().and_then(super::prefs::normalize_hex_color);
                    self.prefs.save();
                }
            }
            Command::SetColumnWidth { key, width } => {
                self.prefs
                    .column_widths
                    .insert(key, width.clamp(32, 420));
                self.prefs.save();
            }
            Command::SetColumnHidden { key, hidden } => {
                let keys = &mut self.prefs.column_hidden;
                keys.retain(|k| k != &key);
                if hidden {
                    keys.push(key);
                }
                self.prefs.save();
            }
            Command::ResetColumns => {
                self.prefs.column_widths.clear();
                self.prefs.column_hidden.clear();
                self.prefs.save();
            }
            Command::SetFont(font) => {
                // An empty value means "system default".
                self.prefs.font = font;
                self.prefs.save();
            }
            Command::SetSidebarWidth(width) => {
                self.prefs.sidebar_width = width
                    .clamp(super::prefs::MIN_SIDEBAR_WIDTH, super::prefs::MAX_SIDEBAR_WIDTH);
                self.prefs.save();
            }
            Command::SetNotifyComplete(on) => {
                self.prefs.notify_complete = on;
                self.prefs.save();
            }
            Command::SetFocusNewDownload(on) => {
                self.prefs.focus_new_download = on;
                self.prefs.save();
            }
            Command::SetFloatingCard(on) => {
                self.prefs.floating_card = on;
                self.prefs.save();
            }
            Command::SetProxyMode(raw) => {
                self.prefs.proxy_mode = super::prefs::normalize_proxy_mode(&raw);
                self.prefs.save();
                self.apply_proxy();
            }
            Command::SetProxyKind(raw) => {
                self.prefs.proxy_kind = super::prefs::normalize_proxy_kind(&raw);
                self.prefs.save();
                self.apply_proxy();
            }
            Command::SetProxyFields { host, port, user, pass } => {
                self.prefs.proxy_host = host;
                self.prefs.proxy_port = port.max(1);
                self.prefs.proxy_user = user;
                self.prefs.proxy_pass = pass;
                // Typing a proxy address is an explicit opt-in; leaving the
                // mode on "No proxy" would silently ignore it.
                if !self.prefs.proxy_host.trim().is_empty()
                    && super::prefs::normalize_proxy_mode(&self.prefs.proxy_mode) == "none"
                {
                    self.prefs.proxy_mode = "manual".into();
                }
                self.prefs.save();
                self.apply_proxy();
            }
            Command::SetProxyProtocols { http, https, ftp } => {
                self.prefs.proxy_http = http;
                self.prefs.proxy_https = https;
                self.prefs.proxy_ftp = ftp;
                self.prefs.save();
                self.apply_proxy();
            }
            Command::SetProxyFromBrowser(on) => {
                self.prefs.proxy_from_browser = on;
                self.prefs.save();
                self.apply_proxy();
            }
            Command::SetBrowserModifier(raw) => {
                self.prefs.browser_modifier = super::prefs::normalize_browser_modifier(&raw);
                super::server::set_browser_modifier(self.prefs.browser_modifier.clone());
                self.prefs.save();
            }
            Command::SetUiLang(raw) => {
                self.prefs.ui_lang = super::prefs::normalize_ui_lang(&raw);
                self.prefs.save();
            }
            Command::SetDirectFormats(raw) => {
                self.prefs.direct_formats = super::prefs::parse_direct_formats(&raw);
                super::server::set_direct_formats(self.prefs.direct_formats.clone());
                self.prefs.save();
            }
            Command::SetSort { col, desc } => {
                self.prefs.sort_col = col;
                self.prefs.sort_desc = desc;
                self.prefs.save();
            }
            Command::AutostartConfigured => {
                self.prefs.autostart_configured = true;
                self.prefs.save();
            }
            Command::SetLastUpdateCheck(secs) => {
                self.prefs.last_update_check = secs;
                self.prefs.save();
            }
            Command::SetUpdateNotified(version) => {
                self.prefs.update_notified = version;
                self.prefs.save();
            }
            Command::CancelShutdown => {
                if self.shutdown_at.take().is_some() {
                    events.push(Event::ShutdownAt(None));
                }
            }
            Command::Shutdown => {
                self.mgr.shutdown();
                self.prefs.save();
            }
        }
        self.view.snaps = self.mgr.list();
        events
    }

    /// Moves downloads into exactly one list.
    fn assign_to_list(&mut self, ids: &[String], name: &str) {
        for l in &mut self.prefs.lists {
            l.items.retain(|id| !ids.iter().any(|x| x == id));
        }
        if let Some(l) = self.prefs.lists.iter_mut().find(|l| l.name == name) {
            l.items.extend(ids.iter().cloned());
        }
        self.prefs.save();
    }

    fn prune_lists(&mut self) {
        let ids: HashSet<String> = self.mgr.list().into_iter().map(|s| s.id).collect();
        let mut changed = false;
        for l in &mut self.prefs.lists {
            let before = l.items.len();
            l.items.retain(|id| ids.contains(id));
            changed |= l.items.len() != before;
        }
        if changed {
            self.prefs.save();
        }
    }

    /// Applies each list's cap and schedule: over the limit (or outside the
    /// schedule window) downloads at the end of the queue pause, and waiting
    /// ones resume in queue order as slots free up.
    fn enforce_lists(&mut self, events: &mut Vec<Event>) {
        let snaps: &[Snapshot] = &self.view.snaps;
        dbg(format!(
            "t={} snaps={:?}",
            now_ms() % 10_000_000,
            snaps
                .iter()
                .map(|s| format!("{}:{}:{}", s.name, s.status as u8, s.received))
                .collect::<Vec<_>>()
        ));
        let live: HashSet<String> = snaps.iter().map(|s| s.id.clone()).collect();
        self.auto_queued.retain(|id| live.contains(id));
        self.user_paused.retain(|id| live.contains(id));
        self.user_started.retain(|id| live.contains(id));
        // Every download lives in a list; anything unlisted falls back to main.
        let default_name = self
            .prefs
            .lists
            .iter()
            .find(|l| l.is_default)
            .map(|l| l.name.clone());
        if let Some(default_name) = default_name {
            let listed: HashSet<&str> = self
                .prefs
                .lists
                .iter()
                .flat_map(|l| l.items.iter().map(|s| s.as_str()))
                .collect();
            let missing: Vec<String> = snaps
                .iter()
                .filter(|s| !listed.contains(s.id.as_str()))
                .map(|s| s.id.clone())
                .collect();
            if !missing.is_empty() {
                if let Some(l) = self.prefs.lists.iter_mut().find(|l| l.name == default_name) {
                    l.items.extend(missing);
                }
                self.prefs.save();
            }
        }
        let now_min = sys::local_minutes();
        let events_sched = schedule_events(&self.prefs.lists, now_min, &mut self.sched_open);
        for id in &events_sched.opened {
            self.user_paused.remove(id);
        }
        if events_sched.shutdown {
            let at = now_ms() + 30_000;
            let next = self.shutdown_at.map_or(at, |cur| cur.min(at));
            self.shutdown_at = Some(next);
            events.push(Event::ShutdownAt(Some(next)));
        }
        let rules = list_rules(&self.prefs.lists, now_min);
        if rules.is_empty() {
            return;
        }
        let (pause, resume) = plan_limit_actions(
            &rules,
            snaps,
            &self.auto_queued,
            &self.user_paused,
            &self.user_started,
        );
        drop(rules);
        if !pause.is_empty() || !resume.is_empty() {
            dbg(format!("t={} plan pause={pause:?} resume={resume:?}", now_ms() % 10_000_000));
        }
        for id in pause {
            self.mgr.pause(&id);
            self.auto_queued.insert(id);
        }
        for id in resume {
            self.mgr.resume(&id);
            self.auto_queued.remove(&id);
        }
        self.view.snaps = self.mgr.list();
    }

}

/// Guarantees a default list and at most one list per download.
/// Returns true when something had to be fixed.
/// Temporary diagnostic used while validating the migration.
fn dbg(msg: String) {
    if std::env::var_os("DVIDM_DEBUG").is_none() {
        return;
    }
    use std::io::Write as _;
    let path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("dvidm-debug.log")))
        .unwrap_or_else(|| PathBuf::from("dvidm-debug.log"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

pub(crate) fn normalize_lists(lists: &mut Vec<DownloadList>) -> bool {
    let mut changed = false;
    match lists.iter().position(|l| l.is_default) {
        Some(keep) => {
            for (i, l) in lists.iter_mut().enumerate() {
                if i != keep && l.is_default {
                    l.is_default = false;
                    changed = true;
                }
            }
        }
        None => {
            if let Some(i) = lists.iter().position(|l| l.name.eq_ignore_ascii_case("main")) {
                lists[i].is_default = true;
            } else {
                let color = (lists.len() % LIST_COLOR_COUNT) as u8;
                lists.insert(
                    0,
                    DownloadList {
                        name: "main".into(),
                        items: Vec::new(),
                        color,
                        color_hex: None,
                        max_parallel: 0,
                        sched_start: None,
                        sched_end: None,
                        shutdown_at_end: false,
                        is_default: true,
                    },
                );
            }
            changed = true;
        }
    }
    let mut seen: HashSet<String> = HashSet::new();
    for l in lists.iter_mut() {
        let before = l.items.len();
        l.items.retain(|id| seen.insert(id.clone()));
        changed |= l.items.len() != before;
    }
    changed
}

/// Best-effort move of partial files out of the old exe-relative temp folder.
fn migrate_temp_files(from: &Path, to: &Path) {
    if from == to || !from.is_dir() {
        return;
    }
    let _ = std::fs::create_dir_all(to);
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let target = to.join(entry.file_name());
        if target.exists() {
            continue;
        }
        if std::fs::rename(&path, &target).is_err() {
            if std::fs::copy(&path, &target).is_ok() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Build the engine proxy URL from the settings fields. A full URL
/// (`socks5://host:port`) is used as typed; otherwise the scheme comes from
/// the proxy-kind selector and the port is only added when the address does
/// not already carry one.
fn build_proxy_url(host: &str, port: u16, kind: &str, user: &str, pass: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    let mut base = if host.contains("://") {
        host.to_string()
    } else {
        let scheme = super::prefs::normalize_proxy_kind(kind);
        let (address, has_port) = split_host_port(host);
        if has_port {
            format!("{scheme}://{address}")
        } else {
            format!("{scheme}://{address}:{}", port.max(1))
        }
    };
    if !user.is_empty() {
        if let Some((scheme, rest)) = base.split_once("://") {
            // ureq's proxy parser reads the credentials verbatim (it does not
            // percent-decode), so pass them through as typed.
            base = format!("{scheme}://{user}:{pass}@{rest}");
        }
    }
    base
}

fn split_host_port(host: &str) -> (String, bool) {
    let host = host.trim();
    if host.starts_with('[') {
        if let Some(end) = host.find(']') {
            let rest = &host[end + 1..];
            return (host.to_string(), rest.starts_with(':') && rest.len() > 1);
        }
    }
    if host.matches(':').count() > 1 {
        // Bare IPv6 address: brackets are required in a URL authority.
        return (format!("[{host}]"), false);
    }
    match host.rsplit_once(':') {
        Some((head, tail))
            if !head.is_empty()
                && !tail.is_empty()
                && tail.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (host.to_string(), true)
        }
        _ => (host.to_string(), false),
    }
}

fn mask_proxy(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    match rest.rsplit_once('@') {
        Some((_, host)) => format!("{scheme}://<auth>@{host}"),
        None => url.to_string(),
    }
}

#[cfg(test)]
mod proxy_url_tests {
    use super::build_proxy_url;

    #[test]
    fn proxy_url_shapes() {
        assert_eq!(
            build_proxy_url("127.0.0.1", 1080, "socks5", "", ""),
            "socks5://127.0.0.1:1080"
        );
        assert_eq!(
            build_proxy_url("127.0.0.1:1081", 9999, "http", "", ""),
            "http://127.0.0.1:1081"
        );
        assert_eq!(
            build_proxy_url("socks5://127.0.0.1:1080", 9999, "http", "", ""),
            "socks5://127.0.0.1:1080"
        );
        assert_eq!(
            build_proxy_url("proxy.local", 8080, "http", "user", "p@ss:1"),
            "http://user:p@ss:1@proxy.local:8080"
        );
        assert_eq!(
            build_proxy_url("[::1]", 1080, "socks5", "", ""),
            "socks5://[::1]:1080"
        );
    }
}
