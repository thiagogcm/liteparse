use std::sync::OnceLock;

use liteparse::ocr_merge::PageComplexityStats;

use crate::handle::{
    LiteParseByteView, bytes_view, free_handle, opaque_handles, slice_out, state_ref,
};
use crate::status::{FfiError, LiteParseStatus, guard};
use crate::views::{LiteParsePageComplexity, views};

pub struct LiteParseComplexity {
    _opaque: [u8; 0],
}

/// Status and handle returned by complexity analysis. The handle is null
/// unless the status is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseComplexityNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseComplexity,
}

opaque_handles! {
    LiteParseComplexity => ComplexityState, "complexity";
}

pub(crate) struct ComplexityState {
    stats: Vec<PageComplexityStats>,
    pages: Vec<LiteParsePageComplexity>,
    json: OnceLock<Result<String, String>>,
}

impl ComplexityState {
    pub(crate) fn new(stats: Vec<PageComplexityStats>) -> Self {
        Self {
            pages: views(&stats),
            stats,
            json: OnceLock::new(),
        }
    }
}

/// Destroy a complexity handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_free(complexity: *mut LiteParseComplexity) {
    unsafe { free_handle(complexity) };
}

/// Borrow the analyzed pages.
///
/// # Safety
///
/// `complexity` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_slice(
    complexity: *const LiteParseComplexity,
    out_len: *mut usize,
) -> *const LiteParsePageComplexity {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(complexity)?.pages.as_slice()))
        })
    }
}

/// Borrow the cached JSON report, or an empty view on failure.
///
/// # Safety
///
/// `complexity` must be live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_json(
    complexity: *const LiteParseComplexity,
) -> LiteParseByteView {
    guard(|| {
        let state = unsafe { state_ref(complexity) }?;
        state
            .json
            .get_or_init(|| {
                serde_json::to_string_pretty(&state.stats).map_err(|error| error.to_string())
            })
            .as_deref()
            .map(|json| bytes_view(json.as_bytes()))
            .map_err(FfiError::serialization)
    })
    .unwrap_or_default()
}
