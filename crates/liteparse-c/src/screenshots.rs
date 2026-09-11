use liteparse::ScreenshotResult;
use liteparse::types::PageError;

use crate::budget::{bytes_of, check_result_bytes};
use crate::handle::{
    Arenas, LiteParseArenas, array_ptr, free_handle, opaque_handles, packed_len, view_of,
    view_state,
};
use crate::records::{
    LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL, LiteParseScreenshot, LiteParseScreenshotRect, flag_bits,
};
use crate::render::RenderedScreenshot;
use crate::status::FfiResult;

/// Rendered pages. Views borrow from the handle until it is freed.
pub struct LiteParseScreenshots {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseScreenshots => ScreenshotsState, "screenshots";
}

/// Rendered pages and the solid rectangles detected on them.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseScreenshotsView {
    pub arenas: LiteParseArenas,
    pub screenshots: *const LiteParseScreenshot,
    pub screenshots_len: usize,
    pub rects: *const LiteParseScreenshotRect,
    pub rects_len: usize,
}

pub(crate) struct ScreenshotsState {
    /// Backing storage for `view`.
    #[allow(dead_code)]
    arenas: Arenas,
    #[allow(dead_code)]
    shots: Vec<LiteParseScreenshot>,
    #[allow(dead_code)]
    rects: Vec<LiteParseScreenshotRect>,
    view: LiteParseScreenshotsView,
}

view_state!(ScreenshotsState => LiteParseScreenshotsView, view);

impl ScreenshotsState {
    pub(crate) fn new(source: &[RenderedScreenshot], errors: &[PageError]) -> FfiResult<Self> {
        let mut arenas = Arenas::default();
        arenas.push_errors(errors);
        let (shots, rects) = pack_screenshots(
            source.iter().map(|shot| (&shot.source, shot.effective_dpi)),
            &mut arenas,
            0,
        )?;
        check_result_bytes(arenas.bytes() + bytes_of(&shots) + bytes_of(&rects))?;
        let view = LiteParseScreenshotsView {
            arenas: arenas.view(),
            screenshots: array_ptr(&shots),
            screenshots_len: shots.len(),
            rects: array_ptr(&rects),
            rects_len: rects.len(),
        };
        Ok(Self {
            arenas,
            shots,
            rects,
            view,
        })
    }
}

pub(crate) fn pack_screenshots<'a>(
    screenshots: impl IntoIterator<Item = (&'a ScreenshotResult, f32)>,
    arenas: &mut Arenas,
    arrays: u64,
) -> FfiResult<(Vec<LiteParseScreenshot>, Vec<LiteParseScreenshotRect>)> {
    let mut shots = Vec::new();
    let mut rects = Vec::new();
    for (shot, effective_dpi) in screenshots {
        let packed = arrays + bytes_of(&shots) + bytes_of(&rects);
        let png = arenas.push_blob(&shot.image_bytes, packed)?;
        let rect_offset = rects.len();
        rects.extend(shot.rects.iter().map(LiteParseScreenshotRect::from));
        shots.push(LiteParseScreenshot {
            png,
            page_number: shot.page_num,
            width: shot.width,
            height: shot.height,
            effective_dpi,
            rect_offset: packed_len(rect_offset),
            rect_count: packed_len(shot.rects.len()),
            flags: flag_bits(&[(shot.is_solid_fill, LITEPARSE_SCREENSHOT_FLAG_SOLID_FILL)]),
        });
    }
    Ok((shots, rects))
}

/// Destroy a screenshots handle. Null is allowed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_free(screenshots: *mut LiteParseScreenshots) {
    unsafe { free_handle(screenshots) };
}

/// Borrow the rendered pages; null for a null handle.
///
/// `screenshots` must be null or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_view(
    screenshots: *const LiteParseScreenshots,
) -> *const LiteParseScreenshotsView {
    unsafe { view_of(screenshots) }
}
