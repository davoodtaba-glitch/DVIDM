//! Toolkit-independent application core.
//!
//! This module is compiled into every user interface binary (currently the
//! egui build and the GPUI Kit build). It contains no windowing code: the
//! download engine, persisted preferences, queue scheduling and the command /
//! event boundary live here so the UIs only render state and send commands.
pub(crate) mod format;
pub(crate) mod log;
pub(crate) mod prefs;
pub(crate) mod proto;
pub(crate) mod schedule;
pub(crate) mod server;
pub(crate) mod service;
pub(crate) mod sys;

// The egui binary still uses its own re-exports, so these are only read by the
// GPUI Kit build.
#[allow(unused_imports)]
pub(crate) use prefs::MAX_LIMIT_KBPS;
#[allow(unused_imports)]
pub(crate) use proto::{Command, Event};
#[allow(unused_imports)]
pub(crate) use service::{normalize_lists, DownloadService};
