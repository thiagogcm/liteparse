use liteparse::ScreenshotResult;

use crate::handle::{free_handle, opaque_handles, slice_out, state_ref};
use crate::render::RenderedScreenshot;
use crate::status::LiteParseStatus;
use crate::views::{LiteParseScreenshot, LiteParseScreenshotRect, views};

pub struct LiteParseScreenshots {
    _opaque: [u8; 0],
}

/// Status and handle returned by screenshot renders. The handle is null
/// unless the status is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseScreenshotsNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseScreenshots,
}

opaque_handles! {
    LiteParseScreenshots => ScreenshotsState, "screenshots";
}

pub(crate) struct ScreenshotsState {
    #[allow(dead_code)]
    source: Vec<RenderedScreenshot>,
    shots: Vec<LiteParseScreenshot>,
    rects: Vec<Vec<LiteParseScreenshotRect>>,
}

impl ScreenshotsState {
    pub(crate) fn new(source: Vec<RenderedScreenshot>) -> Self {
        let (shots, rects) =
            screenshot_views(source.iter().map(|shot| (&shot.source, shot.effective_dpi)));
        Self {
            source,
            shots,
            rects,
        }
    }
}

pub(crate) fn screenshot_views<'a>(
    screenshots: impl IntoIterator<Item = (&'a ScreenshotResult, f32)>,
) -> (Vec<LiteParseScreenshot>, Vec<Vec<LiteParseScreenshotRect>>) {
    screenshots
        .into_iter()
        .map(|(shot, effective_dpi)| {
            (
                LiteParseScreenshot::borrow(shot, effective_dpi),
                views(&shot.rects),
            )
        })
        .unzip()
}

/// Destroy a screenshots handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_free(screenshots: *mut LiteParseScreenshots) {
    unsafe { free_handle(screenshots) };
}

/// Borrow all rendered pages.
///
/// # Safety
///
/// `screenshots` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_slice(
    screenshots: *const LiteParseScreenshots,
    out_len: *mut usize,
) -> *const LiteParseScreenshot {
    unsafe {
        slice_out(out_len, || {
            Ok(Some(state_ref(screenshots)?.shots.as_slice()))
        })
    }
}

/// Borrow one page's detected rectangles.
///
/// # Safety
///
/// `screenshots` must be live and `out_len` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_rects(
    screenshots: *const LiteParseScreenshots,
    index: usize,
    out_len: *mut usize,
) -> *const LiteParseScreenshotRect {
    unsafe {
        slice_out(out_len, || {
            Ok(state_ref(screenshots)?.rects.get(index).map(Vec::as_slice))
        })
    }
}
