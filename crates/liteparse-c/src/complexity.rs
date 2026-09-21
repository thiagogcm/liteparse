use std::sync::OnceLock;

use liteparse::ocr_merge::PageComplexityStats;

use crate::handle::{
    LiteParseByteView, array_ptr, bytes_view, free_handle, opaque_handles, state_ref, view_of,
    view_state, write_out,
};
use crate::records::{LiteParsePageComplexity, views};
use crate::status::{FfiError, LiteParseStatus, boundary};

/// Per-page complexity signals. Views borrow from the handle.
pub struct LiteParseComplexity {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseComplexity => ComplexityState, "complexity";
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseComplexityView {
    pub pages: *const LiteParsePageComplexity,
    pub pages_len: usize,
}

pub(crate) struct ComplexityState {
    stats: Vec<PageComplexityStats>,
    /// Backing storage for `view`.
    #[allow(dead_code)]
    pages: Vec<LiteParsePageComplexity>,
    view: LiteParseComplexityView,
    json: OnceLock<Result<String, String>>,
}

view_state!(ComplexityState => LiteParseComplexityView, view);

impl ComplexityState {
    pub(crate) fn new(stats: Vec<PageComplexityStats>) -> Self {
        let pages: Vec<LiteParsePageComplexity> = views(&stats);
        let view = LiteParseComplexityView {
            pages: array_ptr(&pages),
            pages_len: pages.len(),
        };
        Self {
            stats,
            pages,
            view,
            json: OnceLock::new(),
        }
    }
}

/// Destroy a complexity handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_free(complexity: *mut LiteParseComplexity) {
    unsafe { free_handle(complexity) };
}

/// Borrow the analyzed pages; null for a null handle.
///
/// `complexity` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_view(
    complexity: *const LiteParseComplexity,
) -> *const LiteParseComplexityView {
    unsafe { view_of(complexity) }
}

/// Borrow the cached JSON report.
///
/// `complexity` must be live and `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_to_json(
    complexity: *const LiteParseComplexity,
    out: *mut LiteParseByteView,
) -> LiteParseStatus {
    unsafe { write_out(out, LiteParseByteView::default()) };
    boundary(|| {
        let state = unsafe { state_ref(complexity) }?;
        let json = state
            .json
            .get_or_init(|| {
                serde_json::to_string_pretty(&state.stats).map_err(|error| error.to_string())
            })
            .as_deref()
            .map_err(FfiError::serialization)?;
        unsafe { write_out(out, bytes_view(json.as_bytes())) };
        Ok(())
    })
}
