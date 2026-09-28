use image::ImageEncoder;
use liteparse::render::{find_solid_rects_rgba, is_solid_fill_rgba};
use liteparse::types::{PageError, ScreenshotRect};
use liteparse::{LiteParseConfig as CoreConfig, ScreenshotResult};
use liteparse_pdfium::{Bitmap, Document, Library, Page, RectF, pdfium_sys};

use crate::document::{LiteParseRenderRegion, Source};
use crate::records::LiteParsePageGeometry;
use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR};

const MAX_RENDER_LONG_EDGE_PX: f32 = 30_000.0;
/// Maximum pixels in one planned screenshot bitmap: the region alone for a
/// region render without rectangle detection, the whole page otherwise.
pub const LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE: u64 = 64 * 1024 * 1024;
/// Maximum sum of planned raster bytes in one operation: the RGBA bitmaps
/// of a screenshot call (region-sized for a plain region render), or the
/// rasters of one OCR round.
pub const LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION: u64 = 256 * 1024 * 1024;

fn encode_png(rgba: &[u8], width: u32, height: u32) -> FfiResult<Vec<u8>> {
    let mut png_buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_buf);
    encoder
        .write_image(rgba, width, height, image::ColorType::Rgba8.into())
        .map_err(|e| {
            FfiError::new(
                LITEPARSE_STATUS_PARSE_ERROR,
                format!("PNG encode error: {e}"),
            )
        })?;
    Ok(png_buf)
}

/// The page's crop box, or its media box when none is set: the same fallback
/// the core extractor uses.
pub(crate) fn visible_box(page: &Page<'_, '_>) -> RectF {
    page.view_box().unwrap_or(RectF {
        left: 0.0,
        top: page.height(),
        right: page.width(),
        bottom: 0.0,
    })
}

/// Viewport size and geometry of one live PDFium page.
pub(crate) struct PageFacts {
    pub view_box: RectF,
    pub width: f32,
    pub height: f32,
    /// Geometry and whether its rotation was reportable; `None` when PDFium
    /// reported non-finite values.
    pub geometry: Option<(LiteParsePageGeometry, bool)>,
}

pub(crate) fn page_facts(page: &Page<'_, '_>) -> PageFacts {
    let view_box = visible_box(page);
    let (width, height) = page.viewport_size(&view_box);
    let user_unit = page.user_unit();
    let edges = [
        view_box.left,
        view_box.bottom,
        view_box.right,
        view_box.top,
        user_unit,
    ];
    let geometry = (edges.iter().all(|value| value.is_finite()) && user_unit > 0.0).then(|| {
        let rotation = u32::try_from(page.rotation())
            .ok()
            .filter(|turns| *turns < 4);
        (
            LiteParsePageGeometry {
                box_left: view_box.left,
                box_bottom: view_box.bottom,
                box_right: view_box.right,
                box_top: view_box.top,
                user_unit,
                rotation_quarter_turns: rotation.unwrap_or(0),
            },
            rotation.is_some(),
        )
    });
    PageFacts {
        view_box,
        width,
        height,
        geometry,
    }
}

/// Geometry of the 1-based page `page_number`, when PDFium loads it.
pub(crate) fn page_geometry(
    document: &Document<'_>,
    page_number: usize,
) -> Option<(LiteParsePageGeometry, bool)> {
    let page = document.page(page_number.checked_sub(1)? as i32).ok()?;
    page_facts(&page).geometry
}

/// Run `f` on each selected page (all pages when `pages` is `None`, capped
/// by `max_pages`). Under `continue_on_page_error`, a page whose work fails
/// with a parse error is skipped and recorded; other errors propagate.
pub(crate) fn map_pages<T>(
    document: &Document<'_>,
    pages: Option<Vec<u32>>,
    max_pages: usize,
    config: &CoreConfig,
    mut f: impl FnMut(u32, &Page<'_, '_>) -> FfiResult<T>,
) -> FfiResult<(Vec<T>, Vec<PageError>)> {
    let page_count = document.page_count().max(0) as u32;
    let mut pages = pages.unwrap_or_else(|| (1..=page_count).collect());
    pages.truncate(max_pages);
    let mut out = Vec::with_capacity(pages.len());
    let mut errors = Vec::new();
    for page_num in pages {
        let result = document
            .page(page_num as i32 - 1)
            .map_err(FfiError::from)
            .and_then(|page| f(page_num, &page));
        match result {
            Ok(value) => out.push(value),
            Err(error)
                if config.continue_on_page_error
                    && error.status == LITEPARSE_STATUS_PARSE_ERROR =>
            {
                errors.push(PageError {
                    page_number: page_num,
                    message: error.message,
                });
            }
            Err(error) => return Err(error),
        }
    }
    Ok((out, errors))
}

pub(crate) struct RenderedScreenshot {
    pub(crate) source: ScreenshotResult,
    pub(crate) effective_dpi: f32,
}

pub(crate) fn effective_dpi(requested: f32, width: f32, height: f32) -> f32 {
    let long_edge_pt = width.max(height);
    if long_edge_pt > 0.0 {
        requested.min(MAX_RENDER_LONG_EDGE_PX * 72.0 / long_edge_pt)
    } else {
        requested
    }
}

pub(crate) struct RenderRequest {
    pub(crate) dpi: f32,
    pub(crate) detect_rects: bool,
    pub(crate) render_form_fields: bool,
    pub(crate) region: Option<LiteParseRenderRegion>,
}

impl RenderRequest {
    pub(crate) fn from_config(
        config: &CoreConfig,
        dpi_override: f32,
        region: Option<LiteParseRenderRegion>,
    ) -> FfiResult<Self> {
        let dpi = if dpi_override == 0.0 {
            config.dpi
        } else if dpi_override.is_finite() && dpi_override > 0.0 {
            dpi_override
        } else {
            return Err(FfiError::invalid_argument(
                "dpi_override must be finite and greater than zero; zero keeps the configured DPI",
            ));
        };
        if let Some(region) = region {
            let finite = [region.x, region.y, region.width, region.height]
                .iter()
                .all(|value| value.is_finite());
            if !finite
                || region.width <= 0.0
                || region.height <= 0.0
                || region.x < 0.0
                || region.y < 0.0
            {
                return Err(region_error(region, None));
            }
        }
        Ok(Self {
            dpi,
            detect_rects: config.detect_screenshot_rects,
            render_form_fields: config.render_form_fields,
            region,
        })
    }
}

pub(crate) fn render_pages(
    source: &Source,
    pages: Option<&[u32]>,
    request: &RenderRequest,
) -> FfiResult<(Vec<RenderedScreenshot>, Vec<PageError>)> {
    let config = &source.config;
    let lib = Library::try_init()?;
    let document = source.open(&lib, &source.input)?;
    // Rectangle detection reads the whole page, so only a plain region
    // render is charged for the region alone.
    let region = request.region.filter(|_| !request.detect_rects);
    preflight_pages(&document, pages, config, request.dpi, region, usize::MAX)?;
    let form = request
        .render_form_fields
        .then(|| document.form_environment())
        .flatten();
    if let Some(form) = form.as_ref() {
        form.run_document_actions();
    }
    map_pages(
        &document,
        pages.map(<[u32]>::to_vec),
        usize::MAX,
        config,
        |page_num, page| render_page(page, form.as_ref(), page_num, request),
    )
}

/// Reject oversized screenshot work before the core parse path can allocate
/// its own PDFium bitmaps. The core uses the same DPI cap and rounding.
pub(crate) fn preflight_parse_screenshots(
    document: &Document<'_>,
    pages: Option<&[u32]>,
    config: &CoreConfig,
) -> FfiResult {
    preflight_pages(document, pages, config, config.dpi, None, config.max_pages)
}

fn preflight_pages(
    document: &Document<'_>,
    pages: Option<&[u32]>,
    config: &CoreConfig,
    requested_dpi: f32,
    region: Option<LiteParseRenderRegion>,
    max_pages: usize,
) -> FfiResult {
    let mut used = 0;
    map_pages(
        document,
        pages.map(<[u32]>::to_vec),
        max_pages,
        config,
        |_, page| {
            // An area-less page allocates nothing; its render reports it.
            if !has_area(page) {
                return Ok(());
            }
            let (window, ..) = render_window(page, requested_dpi, region)?;
            used += checked_raster_bytes(window.width as f32, window.height as f32, used)?;
            Ok(())
        },
    )?;
    Ok(())
}

/// Whether `page` has a visible area to render. A crop box that misses the
/// media box leaves none, and a zero-sized bitmap is not a resource limit but
/// a page with no pixels.
fn has_area(page: &Page<'_, '_>) -> bool {
    page.width() > 0.0 && page.height() > 0.0
}

/// The failure of rendering a page with no visible area: it fails that page
/// alone, as a parse error naming it.
pub(crate) fn no_visible_area(page_num: u32) -> FfiError {
    FfiError::new(
        LITEPARSE_STATUS_PARSE_ERROR,
        format!("page {page_num} has no visible area to render"),
    )
}

/// Pixels per point for `page` at `requested_dpi`, after the long-edge cap.
fn render_scale(page: &Page<'_, '_>, requested_dpi: f32) -> f32 {
    let user_unit = page.user_unit();
    let dpi = effective_dpi(
        requested_dpi,
        page.width() * user_unit,
        page.height() * user_unit,
    );
    dpi / 72.0 * user_unit
}

/// Whole-page raster size in pixels at `scale`, rounded as PDFium's render
/// sizes it.
fn raster_size(page: &Page<'_, '_>, scale: f32) -> (f32, f32) {
    (
        (page.width() * scale).round(),
        (page.height() * scale).round(),
    )
}

/// The pixels to rasterize within the whole-page raster (`region`'s, or
/// the whole page), and that raster's size. A page without a
/// representable raster is a resource limit.
fn render_window(
    page: &Page<'_, '_>,
    requested_dpi: f32,
    region: Option<LiteParseRenderRegion>,
) -> FfiResult<(Window, u32, u32)> {
    let scale = render_scale(page, requested_dpi);
    let (width, height) = raster_size(page, scale);
    representable(width, height)?;
    let (width, height) = (width as u32, height as u32);
    let window = region.map_or(Window::whole(width, height), |region| {
        Window::of(region, scale, width, height)
    });
    Ok((window, width, height))
}

/// Reject raster edges PDFium cannot allocate.
fn representable(pixel_width: f32, pixel_height: f32) -> FfiResult {
    let edge = |pixels: f32| pixels.is_finite() && pixels >= 1.0 && pixels < i32::MAX as f32;
    if !edge(pixel_width) || !edge(pixel_height) {
        return Err(FfiError::resource_limit(
            "screenshot raster dimensions are outside the supported range",
        ));
    }
    Ok(())
}

fn checked_raster_bytes(pixel_width: f32, pixel_height: f32, used: u64) -> FfiResult<u64> {
    representable(pixel_width, pixel_height)?;
    let pixels = (pixel_width as u64)
        .checked_mul(pixel_height as u64)
        .ok_or_else(|| FfiError::resource_limit("screenshot pixel count overflow"))?;
    if pixels > LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE {
        return Err(FfiError::resource_limit(format!(
            "screenshot raster of {pixels} pixels exceeds the per-page limit"
        )));
    }
    let bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| FfiError::resource_limit("screenshot byte count overflow"))?;
    if used
        .checked_add(bytes)
        .is_none_or(|total| total > LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION)
    {
        return Err(FfiError::resource_limit(
            "screenshot raster bytes exceed the operation limit",
        ));
    }
    Ok(bytes)
}

#[derive(Clone, Copy)]
struct Window {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Window {
    fn whole(width: u32, height: u32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    fn of(region: LiteParseRenderRegion, scale: f32, width: u32, height: u32) -> Self {
        let (x, right) = pixel_span(region.x, region.width, scale, width);
        let (y, bottom) = pixel_span(region.y, region.height, scale, height);
        Self {
            x,
            y,
            width: right - x,
            height: bottom - y,
        }
    }
}

struct Raster {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl Raster {
    fn read(bitmap: &liteparse_pdfium::Bitmap<'_>, window: Window) -> Self {
        let stride = bitmap.stride() as usize;
        let source = bitmap.buffer();
        let mut rgba = Vec::with_capacity(window.width as usize * window.height as usize * 4);
        for row in window.y..window.y + window.height {
            let start = row as usize * stride + window.x as usize * 4;
            let row = &source[start..start + window.width as usize * 4];
            for pixel in row.as_chunks::<4>().0 {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
            }
        }
        Self {
            rgba,
            width: window.width,
            height: window.height,
        }
    }

    fn crop(&self, window: Window) -> Self {
        let row_bytes = self.width as usize * 4;
        let mut rgba = Vec::with_capacity(window.width as usize * window.height as usize * 4);
        for row in window.y..window.y + window.height {
            let start = row as usize * row_bytes + window.x as usize * 4;
            rgba.extend_from_slice(&self.rgba[start..start + window.width as usize * 4]);
        }
        Self {
            rgba,
            width: window.width,
            height: window.height,
        }
    }

    fn is_solid_fill(&self) -> bool {
        is_solid_fill_rgba(&self.rgba, self.width as usize, self.height as usize)
    }
}

fn render_page(
    page: &liteparse_pdfium::Page<'_, '_>,
    form: Option<&liteparse_pdfium::FormEnvironment<'_, '_>>,
    page_num: u32,
    request: &RenderRequest,
) -> FfiResult<RenderedScreenshot> {
    if !has_area(page) {
        return Err(no_visible_area(page_num));
    }
    let user_unit = page.user_unit();
    let page_width = page.width() * user_unit;
    let page_height = page.height() * user_unit;
    let dpi = effective_dpi(request.dpi, page_width, page_height);
    if let Some(region) = request.region {
        region_fits(region, page_width, page_height)?;
    }
    // Rectangle detection reads the whole page; otherwise only the region
    // is rasterized.
    let plain_region = request.region.filter(|_| !request.detect_rects);
    let raster = rasterize(page, form, request.dpi, plain_region)?;
    let solid = raster.is_solid_fill();
    let (raster, rects, is_solid_fill) = if request.detect_rects {
        let rects = if solid {
            Vec::new()
        } else {
            find_solid_rects_rgba(
                &raster.rgba,
                raster.width as usize,
                raster.height as usize,
                page_width,
                page_height,
            )
        };
        match request.region {
            None => (raster, rects, solid),
            Some(region) => {
                let (window, ..) = render_window(page, request.dpi, Some(region))?;
                let cropped = raster.crop(window);
                let solid = cropped.is_solid_fill();
                let rects = rects
                    .into_iter()
                    .filter_map(|rect| clip_rect(rect, region))
                    .collect();
                (cropped, rects, solid)
            }
        }
    } else {
        (raster, Vec::new(), solid)
    };

    let image_bytes = encode_png(&raster.rgba, raster.width, raster.height)?;
    Ok(RenderedScreenshot {
        source: ScreenshotResult {
            page_num,
            width: raster.width,
            height: raster.height,
            image_bytes,
            is_solid_fill,
            rects,
        },
        effective_dpi: dpi,
    })
}

/// Render `region` of the page, or the whole page, as PDFium's page render
/// does. A region is rendered by placing the page at a negative offset in
/// a region-sized bitmap, so PDFium rasterizes nothing outside it. Size
/// and scale match the crop of a whole-page render; glyph anti-aliasing
/// can differ slightly, since PDFium places glyphs relative to the bitmap
/// origin.
fn rasterize(
    page: &liteparse_pdfium::Page<'_, '_>,
    form: Option<&liteparse_pdfium::FormEnvironment<'_, '_>>,
    requested_dpi: f32,
    region: Option<LiteParseRenderRegion>,
) -> FfiResult<Raster> {
    let (window, width, height) = render_window(page, requested_dpi, region)?;
    // SAFETY: `page` proves the PDFium lock is held for the bitmap's lifetime.
    let bitmap = unsafe { Bitmap::new(window.width as i32, window.height as i32) }?;
    bitmap.fill_rect(0, 0, window.width as i32, window.height as i32, 0xFFFFFFFF);
    let _form_page = form.map(|form| page.notify_form_page_loaded(form));
    page.render_into(
        &bitmap,
        -(window.x as i32),
        -(window.y as i32),
        width as i32,
        height as i32,
        form,
        (pdfium_sys::FPDF_ANNOT | pdfium_sys::FPDF_PRINTING) as i32,
    );
    Ok(Raster::read(
        &bitmap,
        Window::whole(window.width, window.height),
    ))
}

fn region_fits(region: LiteParseRenderRegion, page_width: f32, page_height: f32) -> FfiResult {
    if region.x + region.width <= page_width + f32::EPSILON
        && region.y + region.height <= page_height + f32::EPSILON
    {
        return Ok(());
    }
    Err(region_error(region, Some((page_width, page_height))))
}

fn region_error(region: LiteParseRenderRegion, page: Option<(f32, f32)>) -> FfiError {
    let where_ = match page {
        Some((width, height)) => format!("lie inside the {width}x{height} pt page"),
        None => "have a finite, non-negative origin and a positive size".to_owned(),
    };
    FfiError::invalid_argument(format!(
        "region ({}, {}, {}x{}) must {where_}",
        region.x, region.y, region.width, region.height
    ))
}

/// Round length independently so equal-sized regions have equal pixel sizes.
fn pixel_span(start_pt: f32, length_pt: f32, scale: f32, limit: u32) -> (u32, u32) {
    let limit = limit.max(1);
    let length = ((length_pt * scale).round() as u32).clamp(1, limit);
    let start = ((start_pt * scale).round() as u32).min(limit - length);
    (start, start + length)
}

fn clip_rect(rect: ScreenshotRect, region: LiteParseRenderRegion) -> Option<ScreenshotRect> {
    let left = rect.x.max(region.x);
    let top = rect.y.max(region.y);
    let right = (rect.x + rect.width).min(region.x + region.width);
    let bottom = (rect.y + rect.height).min(region.y + region.height);
    (right > left && bottom > top).then_some(ScreenshotRect {
        x: left - region.x,
        y: top - region.y,
        width: right - left,
        height: bottom - top,
        color: rect.color,
        is_line: rect.is_line,
    })
}

#[cfg(test)]
mod tests {
    use liteparse::stages;
    use liteparse::types::PdfInput;

    use super::*;
    use crate::status::LITEPARSE_STATUS_RESOURCE_LIMIT;

    #[test]
    fn raster_preflight_checks_pixels_and_aggregate_bytes() {
        assert_eq!(
            checked_raster_bytes(612.0, 792.0, 0).unwrap(),
            612 * 792 * 4
        );
        assert_eq!(
            checked_raster_bytes(10_000.0, 10_000.0, 0)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        assert_eq!(
            checked_raster_bytes(4_000.0, 4_000.0, LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
        assert_eq!(
            checked_raster_bytes(f32::INFINITY, 792.0, 0)
                .unwrap_err()
                .status,
            LITEPARSE_STATUS_RESOURCE_LIMIT
        );
    }

    fn open_fixture<'lib>(lib: &'lib Library, fixture: &str) -> Document<'lib> {
        let path = format!(
            "{}/../../integration_tests_data/{fixture}",
            env!("CARGO_MANIFEST_DIR")
        );
        stages::open(lib, &PdfInput::Path(path), None, &[]).unwrap()
    }

    /// Viewport size in points of the fixture's first page.
    fn page_size(fixture: &str) -> (f32, f32) {
        let lib = Library::try_init().unwrap();
        let document = open_fixture(&lib, fixture);
        let page = document.page(0).unwrap();
        let user_unit = page.user_unit();
        (page.width() * user_unit, page.height() * user_unit)
    }

    fn render_png(
        fixture: &str,
        region: LiteParseRenderRegion,
        detect: bool,
        forms: bool,
    ) -> (Vec<u8>, bool) {
        let lib = Library::try_init().unwrap();
        let document = open_fixture(&lib, fixture);
        let form = forms.then(|| document.form_environment()).flatten();
        if let Some(form) = form.as_ref() {
            form.run_document_actions();
        }
        let page = document.page(0).unwrap();
        let request = RenderRequest {
            dpi: 96.0,
            detect_rects: detect,
            render_form_fields: forms,
            region: Some(region),
        };
        let shot = render_page(&page, form.as_ref(), 1, &request).unwrap();
        (shot.source.image_bytes, shot.source.is_solid_fill)
    }

    #[test]
    fn pixel_spans_clamp_to_the_raster() {
        assert_eq!(pixel_span(748.4, 67.4, 1.0, 815), (748, 815));
        assert_eq!(pixel_span(900.0, 10.0, 1.0, 815), (805, 815));
        assert_eq!(pixel_span(0.0, 2000.0, 1.0, 815), (0, 815));
    }

    /// Direct region renders place glyphs relative to the region's origin,
    /// so anti-aliased edges may shift slightly against a crop of the
    /// whole-page render; size, DPI, and content must still agree.
    #[test]
    fn direct_region_render_tracks_the_cropped_page_render() {
        let inked = [
            LiteParseRenderRegion {
                x: 36.4,
                y: 40.3,
                width: 400.2,
                height: 300.7,
            },
            LiteParseRenderRegion {
                x: 83.25,
                y: 119.25,
                width: 30.0,
                height: 66.0,
            },
        ];
        for (fixture, forms) in [
            ("sample.pdf", false),
            ("sample_rotated_90cw.pdf", false),
            ("filled_acroform.pdf", true),
        ] {
            // A region flush with the far page edges.
            let (width, height) = page_size(fixture);
            let edge = LiteParseRenderRegion {
                x: width - 50.3,
                y: height - 40.2,
                width: 50.3,
                height: 40.2,
            };
            for (region, ink) in inked.iter().map(|r| (*r, true)).chain([(edge, false)]) {
                let (cropped, cropped_solid) = render_png(fixture, region, true, forms);
                let (direct, direct_solid) = render_png(fixture, region, false, forms);
                let cropped = image::load_from_memory(&cropped).unwrap().to_rgba8();
                let direct = image::load_from_memory(&direct).unwrap().to_rgba8();
                assert_eq!(cropped.dimensions(), direct.dimensions(), "{fixture}");
                if ink {
                    assert!(!cropped_solid && !direct_solid, "{fixture} region has ink");
                }
                let pixels = cropped.pixels().len();
                let pairs = || cropped.pixels().zip(direct.pixels());
                let differing = pairs().filter(|(a, b)| a != b).count();
                let far = pairs()
                    .filter(|(a, b)| a.0.iter().zip(b.0).any(|(x, y)| x.abs_diff(y) > 64))
                    .count();
                assert!(
                    differing * 10 < pixels,
                    "{fixture}: {differing}/{pixels} differ"
                );
                assert!(
                    far * 30 < pixels,
                    "{fixture}: {far}/{pixels} differ by more than 64"
                );
            }
        }
    }
}
