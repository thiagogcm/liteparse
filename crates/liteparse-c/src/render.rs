use liteparse::extract::encode_png;
use liteparse::render::{find_solid_rects_rgba, is_solid_fill_rgba};
use liteparse::types::{PdfInput, ScreenshotRect};
use liteparse::{LiteParseConfig as CoreConfig, ScreenshotResult};
use liteparse_pdfium::{Document, Library};

use crate::document::LiteParseRenderRegion;
use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR};

/// Must match the core renderer's limit.
const MAX_RENDER_LONG_EDGE_PX: f32 = 30_000.0;

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

pub(crate) struct RenderRequest<'a> {
    pub(crate) dpi: f32,
    pub(crate) password: Option<&'a str>,
    pub(crate) detect_rects: bool,
    pub(crate) render_form_fields: bool,
    pub(crate) continue_on_page_error: bool,
    pub(crate) region: Option<LiteParseRenderRegion>,
}

impl<'a> RenderRequest<'a> {
    pub(crate) fn from_config(
        config: &'a CoreConfig,
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
            password: config.password.as_deref(),
            detect_rects: config.detect_screenshot_rects,
            render_form_fields: config.render_form_fields,
            continue_on_page_error: config.continue_on_page_error,
            region,
        })
    }
}

pub(crate) fn load_document<'lib>(
    lib: &'lib Library,
    input: &PdfInput,
    password: Option<&str>,
) -> FfiResult<Document<'lib>> {
    Ok(match input {
        PdfInput::Path(path) => lib.load_document(path, password)?,
        PdfInput::Bytes(data) => lib.load_document_from_bytes(data, password)?,
    })
}

pub(crate) fn render_pages(
    input: &PdfInput,
    pages: Option<&[u32]>,
    request: &RenderRequest<'_>,
) -> FfiResult<Vec<RenderedScreenshot>> {
    let lib = Library::init();
    let document = load_document(&lib, input, request.password)?;
    let page_count = document.page_count().max(0) as u32;
    let pages: Vec<u32> = match pages {
        Some(pages) => pages.to_vec(),
        None => (1..=page_count).collect(),
    };

    let form = request
        .render_form_fields
        .then(|| document.form_environment())
        .flatten();
    if let Some(form) = form.as_ref() {
        form.run_document_actions();
    }

    let mut results = Vec::with_capacity(pages.len());
    for page_num in pages {
        let rendered = (|| -> FfiResult<RenderedScreenshot> {
            if page_num < 1 || page_num > page_count {
                return Err(FfiError::invalid_argument(format!(
                    "page {page_num} out of range (document has {page_count} pages)"
                )));
            }
            let page = document.page((page_num - 1) as i32)?;
            render_page(&page, form.as_ref(), page_num, request)
        })();

        match rendered {
            Ok(rendered) => results.push(rendered),
            Err(error)
                if request.continue_on_page_error
                    && error.status == LITEPARSE_STATUS_PARSE_ERROR =>
            {
                eprintln!(
                    "[render] page {page_num} failed: {} — skipping its screenshot (continue_on_page_error)",
                    error.message
                )
            }
            Err(error) => return Err(error),
        }
    }
    Ok(results)
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
            for pixel in row.chunks_exact(4) {
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
    request: &RenderRequest<'_>,
) -> FfiResult<RenderedScreenshot> {
    let user_unit = page.user_unit();
    let page_width = page.width() * user_unit;
    let page_height = page.height() * user_unit;
    let dpi = effective_dpi(request.dpi, page_width, page_height);
    if let Some(region) = request.region {
        region_fits(region, page_width, page_height)?;
    }

    let bitmap = page.render_with_form(dpi, form)?;
    let whole = Window::whole(bitmap.width() as u32, bitmap.height() as u32);
    let window = request.region.map(|region| {
        let scale = dpi / 72.0 * user_unit;
        Window::of(region, scale, whole.width, whole.height)
    });

    let page_raster =
        (request.detect_rects || window.is_none()).then(|| Raster::read(&bitmap, whole));
    let solid_page = page_raster.as_ref().is_some_and(Raster::is_solid_fill);
    let page_rects = match &page_raster {
        Some(raster) if request.detect_rects && !solid_page => find_solid_rects_rgba(
            &raster.rgba,
            raster.width as usize,
            raster.height as usize,
            page_width,
            page_height,
        ),
        _ => Vec::new(),
    };

    let (raster, rects, is_solid_fill) = match (window, request.region) {
        (None, _) | (_, None) => (
            page_raster.expect("a whole-page render always reads the page"),
            page_rects,
            solid_page,
        ),
        (Some(window), Some(region)) => {
            let cropped = match &page_raster {
                Some(page) => page.crop(window),
                None => Raster::read(&bitmap, window),
            };
            let solid = cropped.is_solid_fill();
            let rects = page_rects
                .into_iter()
                .filter_map(|rect| clip_rect(rect, region))
                .collect();
            (cropped, rects, solid)
        }
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
