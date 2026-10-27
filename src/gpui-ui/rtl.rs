//! Interface direction (RTL) shared by every page.
//!
//! GPUI has no `dir=rtl`. Horizontal order, start/end padding, and ellipsis
//! are mirrored here. The flag is process-wide so pages do not thread direction
//! through every builder. Play/download icons keep their inherent direction.

use gpui_kit::prelude::*;
use gpui_kit::{Div, Pixels, Stateful};
use std::sync::atomic::{AtomicBool, Ordering};

static RTL: AtomicBool = AtomicBool::new(false);

pub(crate) fn set(on: bool) {
    RTL.store(on, Ordering::SeqCst);
}

pub(crate) fn on() -> bool {
    RTL.load(Ordering::SeqCst)
}

/// Switch knob: on sits at the end of the track (right in LTR, left in RTL).
pub(crate) fn switch_thumb_ml(on_state: bool) -> f32 {
    match (on(), on_state) {
        (false, false) => 2.,
        (false, true) => 16.,
        (true, false) => 16.,
        (true, true) => 2.,
    }
}

pub(crate) trait Mirror {
    fn mirror(self) -> Self;
}

impl Mirror for Div {
    fn mirror(self) -> Self {
        if on() {
            self.flex_row_reverse()
        } else {
            self
        }
    }
}

impl Mirror for Stateful<Div> {
    fn mirror(self) -> Self {
        if on() {
            self.flex_row_reverse()
        } else {
            self
        }
    }
}

pub(crate) trait AlignEnd {
    fn align_end(self) -> Self;
}

impl AlignEnd for Div {
    fn align_end(self) -> Self {
        if on() {
            self.text_right()
        } else {
            self
        }
    }
}

impl AlignEnd for Stateful<Div> {
    fn align_end(self) -> Self {
        if on() {
            self.text_right()
        } else {
            self
        }
    }
}

/// Logical start/end edges. In RTL, start is right and end is left.
pub(crate) trait DirEdge {
    fn padding_start(self, v: Pixels) -> Self;
    fn padding_end(self, v: Pixels) -> Self;
    fn margin_start(self, v: Pixels) -> Self;
    #[allow(dead_code)]
    fn margin_end(self, v: Pixels) -> Self;
    fn edge_start(self, v: Pixels) -> Self;
    fn edge_end(self, v: Pixels) -> Self;
    #[allow(dead_code)]
    fn auto_start(self) -> Self;
    #[allow(dead_code)]
    fn auto_end(self) -> Self;
    fn ellipsize_ui(self) -> Self;
}

macro_rules! impl_dir_edge {
    ($ty:ty) => {
        impl DirEdge for $ty {
            fn padding_start(self, v: Pixels) -> Self {
                if on() {
                    self.pr(v)
                } else {
                    self.pl(v)
                }
            }
            fn padding_end(self, v: Pixels) -> Self {
                if on() {
                    self.pl(v)
                } else {
                    self.pr(v)
                }
            }
            fn margin_start(self, v: Pixels) -> Self {
                if on() {
                    self.mr(v)
                } else {
                    self.ml(v)
                }
            }
            fn margin_end(self, v: Pixels) -> Self {
                if on() {
                    self.ml(v)
                } else {
                    self.mr(v)
                }
            }
            fn edge_start(self, v: Pixels) -> Self {
                if on() {
                    self.right(v)
                } else {
                    self.left(v)
                }
            }
            fn edge_end(self, v: Pixels) -> Self {
                if on() {
                    self.left(v)
                } else {
                    self.right(v)
                }
            }
            fn auto_start(self) -> Self {
                if on() {
                    self.mr_auto()
                } else {
                    self.ml_auto()
                }
            }
            fn auto_end(self) -> Self {
                if on() {
                    self.ml_auto()
                } else {
                    self.mr_auto()
                }
            }
            fn ellipsize_ui(self) -> Self {
                if on() {
                    self.overflow_hidden().text_right()
                } else {
                    self.overflow_hidden().text_ellipsis()
                }
            }
        }
    };
}

impl_dir_edge!(Div);
impl_dir_edge!(Stateful<Div>);
