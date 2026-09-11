use std::ptr;

use liteparse::LiteParseConfig as CoreConfig;
use liteparse::config::{CropBox, ImageMode, OutputFormat};

use crate::handle::{LiteParseByteView, as_slice, optional_view_str, required_view_str};
use crate::status::{FfiError, FfiResult};

/// Values for `LiteParseConfig.output_format`.
pub const LITEPARSE_OUTPUT_FORMAT_JSON: u32 = 0;
pub const LITEPARSE_OUTPUT_FORMAT_TEXT: u32 = 1;
pub const LITEPARSE_OUTPUT_FORMAT_MARKDOWN: u32 = 2;

/// Values for `LiteParseConfig.image_mode`.
pub const LITEPARSE_IMAGE_MODE_OFF: u32 = 0;
pub const LITEPARSE_IMAGE_MODE_PLACEHOLDER: u32 = 1;
pub const LITEPARSE_IMAGE_MODE_EMBED: u32 = 2;

/// Keep the native default in fields where zero is not meaningful.
pub const LITEPARSE_UNSET: u32 = u32::MAX;

/// Bits are ABI-stable: append new flags without renumbering existing ones.
pub const LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR: u64 = 1u64 << 0;
pub const LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS: u64 = 1u64 << 1;
pub const LITEPARSE_FLAG_EMIT_WORD_BOXES: u64 = 1u64 << 2;
pub const LITEPARSE_FLAG_EXTRACT_ANNOTATIONS: u64 = 1u64 << 3;
pub const LITEPARSE_FLAG_EXTRACT_BLOCKS: u64 = 1u64 << 4;
pub const LITEPARSE_FLAG_EXTRACT_CONTENT_BOUNDS: u64 = 1u64 << 5;
pub const LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA: u64 = 1u64 << 6;
pub const LITEPARSE_FLAG_EXTRACT_FORM_FIELDS: u64 = 1u64 << 7;
pub const LITEPARSE_FLAG_EXTRACT_IMAGES: u64 = 1u64 << 8;
pub const LITEPARSE_FLAG_EXTRACT_LINKS: u64 = 1u64 << 9;
pub const LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE: u64 = 1u64 << 10;
pub const LITEPARSE_FLAG_EXTRACT_TEXT_METADATA: u64 = 1u64 << 11;
pub const LITEPARSE_FLAG_EXTRACT_VECTOR_GRAPHICS: u64 = 1u64 << 12;
pub const LITEPARSE_FLAG_EXTRACT_XFA_PACKETS: u64 = 1u64 << 13;
pub const LITEPARSE_FLAG_INCLUDE_COMPLEXITY: u64 = 1u64 << 14;
pub const LITEPARSE_FLAG_KEEP_HEADERS_FOOTERS: u64 = 1u64 << 15;
pub const LITEPARSE_FLAG_OCR_ENABLED: u64 = 1u64 << 16;
pub const LITEPARSE_FLAG_OCR_FAILURE_FATAL: u64 = 1u64 << 17;
pub const LITEPARSE_FLAG_PRESERVE_VERY_SMALL_TEXT: u64 = 1u64 << 18;
pub const LITEPARSE_FLAG_QUIET: u64 = 1u64 << 19;
pub const LITEPARSE_FLAG_RENDER_FORM_FIELDS: u64 = 1u64 << 20;
pub const LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT: u64 = 1u64 << 21;
pub const LITEPARSE_FLAG_EXTRACT_SCREENSHOTS: u64 = 1u64 << 22;

// The destructure and mask assertion make omitted core booleans and flag gaps
// compile-time failures.
macro_rules! config_flags {
    ($($name:ident => $field:ident),* $(,)?) => {
        fn apply_flags(set: u64, values: u64, config: &mut CoreConfig) {
            $(if set & $name != 0 {
                config.$field = values & $name != 0;
            })*
        }

        const _: () = {
            const TABLE_MASK: u64 = 0 $(| $name)*;
            const COUNT: u32 = [$(stringify!($name)),*].len() as u32;
            assert!(
                TABLE_MASK == (1u64 << COUNT) - 1,
                "config_flags! must list every LITEPARSE_FLAG_* exactly once"
            );
        };

        #[allow(dead_code)]
        fn _core_bool_fields_are_mirrored(config: &CoreConfig) {
            let CoreConfig {
                $($field: _,)*
                ocr_language: _,
                ocr_server_url: _,
                ocr_server_headers: _,
                tessdata_path: _,
                max_pages: _,
                target_pages: _,
                dpi: _,
                output_format: _,
                password: _,
                num_workers: _,
                image_mode: _,
                image_output_dir: _,
                ocr_hedge_delays_ms: _,
                crop_box: _,
            } = config;
        }
    };
}

config_flags! {
    LITEPARSE_FLAG_CONTINUE_ON_PAGE_ERROR => continue_on_page_error,
    LITEPARSE_FLAG_DETECT_SCREENSHOT_RECTS => detect_screenshot_rects,
    LITEPARSE_FLAG_EMIT_WORD_BOXES => emit_word_boxes,
    LITEPARSE_FLAG_EXTRACT_ANNOTATIONS => extract_annotations,
    LITEPARSE_FLAG_EXTRACT_BLOCKS => extract_blocks,
    LITEPARSE_FLAG_EXTRACT_CONTENT_BOUNDS => extract_content_bounds,
    LITEPARSE_FLAG_EXTRACT_DOCUMENT_METADATA => extract_document_metadata,
    LITEPARSE_FLAG_EXTRACT_FORM_FIELDS => extract_form_fields,
    LITEPARSE_FLAG_EXTRACT_IMAGES => extract_images,
    LITEPARSE_FLAG_EXTRACT_LINKS => extract_links,
    LITEPARSE_FLAG_EXTRACT_STRUCTURE_TREE => extract_structure_tree,
    LITEPARSE_FLAG_EXTRACT_TEXT_METADATA => extract_text_metadata,
    LITEPARSE_FLAG_EXTRACT_VECTOR_GRAPHICS => extract_vector_graphics,
    LITEPARSE_FLAG_EXTRACT_XFA_PACKETS => extract_xfa_packets,
    LITEPARSE_FLAG_INCLUDE_COMPLEXITY => include_complexity,
    LITEPARSE_FLAG_KEEP_HEADERS_FOOTERS => keep_headers_footers,
    LITEPARSE_FLAG_OCR_ENABLED => ocr_enabled,
    LITEPARSE_FLAG_OCR_FAILURE_FATAL => ocr_failure_fatal,
    LITEPARSE_FLAG_PRESERVE_VERY_SMALL_TEXT => preserve_very_small_text,
    LITEPARSE_FLAG_QUIET => quiet,
    LITEPARSE_FLAG_RENDER_FORM_FIELDS => render_form_fields,
    LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT => skip_diagonal_text,
    LITEPARSE_FLAG_EXTRACT_SCREENSHOTS => extract_screenshots,
}

#[allow(dead_code)]
fn _core_enum_variants_are_mirrored(format: OutputFormat, mode: ImageMode) {
    match format {
        OutputFormat::Json | OutputFormat::Text | OutputFormat::Markdown => {}
    }
    match mode {
        ImageMode::Off | ImageMode::Placeholder | ImageMode::Embed => {}
    }
}

/// One HTTP OCR header, copied by `liteparse_parser_new`.
#[repr(C)]
pub struct LiteParseHeader {
    pub name: LiteParseByteView,
    pub value: LiteParseByteView,
}

/// Start with `liteparse_config_default`; parser creation copies all views.
#[repr(C)]
pub struct LiteParseConfig {
    /// Must equal `sizeof(LiteParseConfig)`.
    pub size_of_config: usize,
    pub bools_set: u64,
    pub bools_values: u64,
    /// Zero keeps the native default (1000).
    pub max_pages: usize,
    /// Zero keeps the native default.
    pub num_workers: usize,
    /// Zero keeps the native default; nonzero values must be finite and > 0.
    pub dpi: f32,
    /// `LITEPARSE_UNSET` keeps the native default.
    pub output_format: u32,
    /// `LITEPARSE_UNSET` keeps the native default.
    pub image_mode: u32,
    /// Normalized fractions ordered top, right, bottom, left. When
    /// `has_crop_box` is set every value must lie in `[0, 1]` with
    /// `top + bottom < 1` and `left + right < 1`.
    pub crop_box: [f32; 4],
    pub has_crop_box: bool,
    pub ocr_language: LiteParseByteView,
    pub ocr_server_url: LiteParseByteView,
    pub tessdata_path: LiteParseByteView,
    pub password: LiteParseByteView,
    pub image_output_dir: LiteParseByteView,
    pub ocr_server_headers: *const LiteParseHeader,
    pub ocr_server_headers_len: usize,
    pub ocr_hedge_delays_ms: *const u64,
    pub ocr_hedge_delays_ms_len: usize,
    /// Optional `%02x%02x.msgpack` glyph-database directory. An explicit path
    /// overrides `LITEPARSE_FONT_DB_DIR`.
    pub font_db_dir: LiteParseByteView,
}

#[unsafe(no_mangle)]
pub extern "C" fn liteparse_config_default() -> LiteParseConfig {
    LiteParseConfig {
        size_of_config: size_of::<LiteParseConfig>(),
        bools_set: 0,
        bools_values: 0,
        max_pages: 0,
        num_workers: 0,
        dpi: 0.0,
        output_format: LITEPARSE_UNSET,
        image_mode: LITEPARSE_UNSET,
        crop_box: [0.0; 4],
        has_crop_box: false,
        ocr_language: LiteParseByteView::default(),
        ocr_server_url: LiteParseByteView::default(),
        tessdata_path: LiteParseByteView::default(),
        password: LiteParseByteView::default(),
        image_output_dir: LiteParseByteView::default(),
        ocr_server_headers: ptr::null(),
        ocr_server_headers_len: 0,
        ocr_hedge_delays_ms: ptr::null(),
        ocr_hedge_delays_ms_len: 0,
        font_db_dir: LiteParseByteView::default(),
    }
}

fn crop_box(fractions: [f32; 4]) -> FfiResult<CropBox> {
    let [top, right, bottom, left] = fractions;
    let in_unit_range = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
    if !fractions.iter().copied().all(in_unit_range) || top + bottom >= 1.0 || left + right >= 1.0 {
        return Err(FfiError::invalid_config(
            "crop_box fractions must be finite, within [0, 1], and leave at least one \
             uncovered axis",
        ));
    }
    Ok(CropBox {
        top,
        right,
        bottom,
        left,
    })
}

pub(crate) struct OwnedParserConfig {
    pub core: CoreConfig,
    pub font_db_dir: Option<std::path::PathBuf>,
}

pub(crate) unsafe fn owned_config(raw: *const LiteParseConfig) -> FfiResult<OwnedParserConfig> {
    let raw = unsafe { raw.as_ref() }.ok_or_else(|| {
        FfiError::invalid_argument("config must not be null; start from liteparse_config_default()")
    })?;
    if raw.size_of_config != size_of::<LiteParseConfig>() {
        return Err(FfiError::invalid_argument(format!(
            "config.size_of_config is {} but this library expects {}; rebuild against the current header",
            raw.size_of_config,
            size_of::<LiteParseConfig>()
        )));
    }

    let mut config = CoreConfig::default();
    apply_flags(raw.bools_set, raw.bools_values, &mut config);

    if raw.max_pages != 0 {
        config.max_pages = raw.max_pages;
    }
    if raw.num_workers != 0 {
        config.num_workers = raw.num_workers;
    }
    if raw.dpi != 0.0 {
        if !raw.dpi.is_finite() || raw.dpi <= 0.0 {
            return Err(FfiError::invalid_config(
                "dpi must be finite and greater than zero",
            ));
        }
        config.dpi = raw.dpi;
    }
    if raw.output_format != LITEPARSE_UNSET {
        config.output_format = match raw.output_format {
            LITEPARSE_OUTPUT_FORMAT_JSON => OutputFormat::Json,
            LITEPARSE_OUTPUT_FORMAT_TEXT => OutputFormat::Text,
            LITEPARSE_OUTPUT_FORMAT_MARKDOWN => OutputFormat::Markdown,
            _ => return Err(FfiError::invalid_config("unknown output format")),
        };
    }
    if raw.image_mode != LITEPARSE_UNSET {
        config.image_mode = match raw.image_mode {
            LITEPARSE_IMAGE_MODE_OFF => ImageMode::Off,
            LITEPARSE_IMAGE_MODE_PLACEHOLDER => ImageMode::Placeholder,
            LITEPARSE_IMAGE_MODE_EMBED => ImageMode::Embed,
            _ => return Err(FfiError::invalid_config("unknown image mode")),
        };
    }
    if raw.has_crop_box {
        config.crop_box = Some(crop_box(raw.crop_box)?);
    }

    unsafe {
        if let Some(v) = optional_view_str(raw.ocr_language, "ocr_language")? {
            config.ocr_language = v;
        }
        config.ocr_server_url = optional_view_str(raw.ocr_server_url, "ocr_server_url")?;
        config.tessdata_path = optional_view_str(raw.tessdata_path, "tessdata_path")?;
        config.password = optional_view_str(raw.password, "password")?;
        config.image_output_dir = optional_view_str(raw.image_output_dir, "image_output_dir")?;
    }

    let font_db_dir = unsafe { optional_view_str(raw.font_db_dir, "font_db_dir") }?
        .filter(|dir| !dir.is_empty())
        .map(std::path::PathBuf::from);

    let headers = unsafe {
        as_slice(
            raw.ocr_server_headers,
            raw.ocr_server_headers_len,
            "ocr_server_headers",
        )
    }?;
    for header in headers.unwrap_or_default() {
        let name = unsafe { required_view_str(header.name, "header name") }?;
        let value = unsafe { required_view_str(header.value, "header value") }?;
        config.ocr_server_headers.push((name, value));
    }

    let delays = unsafe {
        as_slice(
            raw.ocr_hedge_delays_ms,
            raw.ocr_hedge_delays_ms_len,
            "ocr_hedge_delays_ms",
        )
    }?;
    config
        .ocr_hedge_delays_ms
        .extend_from_slice(delays.unwrap_or_default());

    if config.image_output_dir.is_some() && !config.effective_extract_images() {
        return Err(FfiError::invalid_config(
            "image_output_dir requires extract_images = true or image_mode = \"embed\"",
        ));
    }
    Ok(OwnedParserConfig {
        core: config,
        font_db_dir,
    })
}
