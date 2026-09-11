use std::ptr;

use liteparse::LiteParseConfig as CoreConfig;
use liteparse::config::{CropBox, ImageMode, OutputFormat, PageOrientationCorrection};

use crate::handle::{
    LiteParseByteView, SizedInput, as_slice, optional_view_str, required_view_str, sized_input,
};
use crate::status::{FfiError, FfiResult};

/// Values for `LiteParseConfig.output_format`.
pub const LITEPARSE_OUTPUT_FORMAT_TEXT: u32 = 0;
pub const LITEPARSE_OUTPUT_FORMAT_MARKDOWN: u32 = 1;

/// Values for `LiteParseConfig.image_mode`.
pub const LITEPARSE_IMAGE_MODE_OFF: u32 = 0;
pub const LITEPARSE_IMAGE_MODE_PLACEHOLDER: u32 = 1;
pub const LITEPARSE_IMAGE_MODE_EMBED: u32 = 2;

/// Default resolution in DPI for raster rendering and screenshots.
pub const LITEPARSE_DEFAULT_DPI: f32 = 150.0;

/// `LiteParseConfig.options` bit: apply `crop_box`. Presence bits count down
/// from bit 63; boolean flags count up from bit 0.
pub const LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX: u64 = 1u64 << 63;

/// `LiteParseConfig.options` bits, whose presence denotes true.
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
pub const LITEPARSE_FLAG_RENDER_FORM_FIELDS: u64 = 1u64 << 19;
pub const LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT: u64 = 1u64 << 20;
pub const LITEPARSE_FLAG_EXTRACT_SCREENSHOTS: u64 = 1u64 << 21;

// The destructure and mask assertion make omitted core booleans and flag gaps
// compile-time failures.
macro_rules! config_flags {
    ($($name:ident => $field:ident),* $(,)?) => {
        const KNOWN_BOOL_FLAGS: u64 = 0 $(| $name)*;

        fn apply_flags(options: u64, config: &mut CoreConfig) {
            $(config.$field = options & $name != 0;)*
        }

        fn default_flags(config: &CoreConfig) -> u64 {
            0 $(| if config.$field { $name } else { 0 })*
        }

        const _: () = {
            const COUNT: u32 = [$(stringify!($name)),*].len() as u32;
            assert!(
                KNOWN_BOOL_FLAGS == (1u64 << COUNT) - 1,
                "config_flags! must list every LITEPARSE_FLAG_* exactly once"
            );
            assert!(
                KNOWN_BOOL_FLAGS & LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX == 0,
                "boolean flags must not reach the presence bits"
            );
        };

        #[allow(dead_code)]
        fn _core_bool_fields_are_mirrored(config: &CoreConfig) {
            let CoreConfig {
                $($field: _,)*
                quiet: _,
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
                page_orientation_corrections: _,
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
    LITEPARSE_FLAG_RENDER_FORM_FIELDS => render_form_fields,
    LITEPARSE_FLAG_SKIP_DIAGONAL_TEXT => skip_diagonal_text,
    LITEPARSE_FLAG_EXTRACT_SCREENSHOTS => extract_screenshots,
}

/// One HTTP OCR header, copied by `liteparse_parser_new`.
#[repr(C)]
pub struct LiteParseHeader {
    pub name: LiteParseByteView,
    pub value: LiteParseByteView,
}

/// One per-page orientation correction: `page` is 1-based, `angle` is the
/// clockwise degrees (0/90/180/270) the content appears rotated. Out-of-range
/// pages are ignored like core.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LiteParsePageOrientationCorrection {
    pub page: u32,
    pub angle: u32,
}

/// Start with `liteparse_config_init`; parser creation copies all views.
#[repr(C)]
pub struct LiteParseConfig {
    /// Must equal `sizeof(LiteParseConfig)`.
    pub struct_size: u32,
    /// Actual boolean values (`LITEPARSE_FLAG_*`) and crop-box presence.
    pub options: u64,
    /// Zero parses no pages.
    pub max_pages: u32,
    pub num_workers: u32,
    pub output_format: u32,
    pub image_mode: u32,
    /// Must be finite and greater than zero.
    pub dpi: f32,
    /// Normalized fractions ordered top, right, bottom, left, applied when
    /// `LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX` is set in `options`. Every value must lie in
    /// `[0, 1]` with `top + bottom < 1` and `left + right < 1`.
    pub crop_box: [f32; 4],
    pub ocr_language: LiteParseByteView,
    pub ocr_server_url: LiteParseByteView,
    pub tessdata_path: LiteParseByteView,
    pub password: LiteParseByteView,
    /// Optional `%02x%02x.msgpack` glyph-database directory. An explicit path
    /// overrides `LITEPARSE_FONT_DB_DIR`.
    pub font_db_dir: LiteParseByteView,
    pub ocr_server_headers: *const LiteParseHeader,
    pub ocr_server_headers_len: usize,
    pub ocr_hedge_delays_ms: *const u64,
    pub ocr_hedge_delays_ms_len: usize,
    pub orientation_corrections: *const LiteParsePageOrientationCorrection,
    pub orientation_corrections_len: usize,
}

/// Fill `config` with defaults. Null is a no-op.
///
/// `config` must be null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_config_init(config: *mut LiteParseConfig) {
    let defaults = CoreConfig::default();
    let value = LiteParseConfig {
        struct_size: size_of::<LiteParseConfig>() as u32,
        options: default_flags(&defaults),
        max_pages: defaults.max_pages as u32,
        num_workers: defaults.num_workers as u32,
        output_format: match defaults.output_format {
            OutputFormat::Json | OutputFormat::Text => LITEPARSE_OUTPUT_FORMAT_TEXT,
            OutputFormat::Markdown => LITEPARSE_OUTPUT_FORMAT_MARKDOWN,
        },
        image_mode: match defaults.image_mode {
            ImageMode::Off => LITEPARSE_IMAGE_MODE_OFF,
            ImageMode::Placeholder => LITEPARSE_IMAGE_MODE_PLACEHOLDER,
            ImageMode::Embed => LITEPARSE_IMAGE_MODE_EMBED,
        },
        dpi: defaults.dpi,
        crop_box: [0.0; 4],
        ocr_language: LiteParseByteView::default(),
        ocr_server_url: LiteParseByteView::default(),
        tessdata_path: LiteParseByteView::default(),
        password: LiteParseByteView::default(),
        font_db_dir: LiteParseByteView::default(),
        ocr_server_headers: ptr::null(),
        ocr_server_headers_len: 0,
        ocr_hedge_delays_ms: ptr::null(),
        ocr_hedge_delays_ms_len: 0,
        orientation_corrections: ptr::null(),
        orientation_corrections_len: 0,
    };
    unsafe { crate::handle::write_out(config, value) };
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

impl SizedInput for LiteParseConfig {
    const NAME: &'static str = "config";
    fn struct_size(&self) -> u32 {
        self.struct_size
    }
}

pub(crate) struct OwnedParserConfig {
    pub core: CoreConfig,
    pub font_db_dir: Option<std::path::PathBuf>,
}

pub(crate) unsafe fn owned_config(raw: *const LiteParseConfig) -> FfiResult<OwnedParserConfig> {
    let raw = unsafe { sized_input(raw) }?;
    if raw.options & !(KNOWN_BOOL_FLAGS | LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX) != 0 {
        return Err(FfiError::invalid_config(
            "config.options contains unknown bits",
        ));
    }

    let mut config = CoreConfig::default();
    apply_flags(raw.options, &mut config);
    // The library writes no progress output.
    config.quiet = true;

    config.max_pages = raw.max_pages as usize;
    config.num_workers = raw.num_workers as usize;
    if !raw.dpi.is_finite() || raw.dpi <= 0.0 {
        return Err(FfiError::invalid_config(
            "dpi must be finite and greater than zero",
        ));
    }
    config.dpi = raw.dpi;
    config.output_format = match raw.output_format {
        LITEPARSE_OUTPUT_FORMAT_TEXT => OutputFormat::Text,
        LITEPARSE_OUTPUT_FORMAT_MARKDOWN => OutputFormat::Markdown,
        _ => return Err(FfiError::invalid_config("unknown output format")),
    };
    config.image_mode = match raw.image_mode {
        LITEPARSE_IMAGE_MODE_OFF => ImageMode::Off,
        LITEPARSE_IMAGE_MODE_PLACEHOLDER => ImageMode::Placeholder,
        LITEPARSE_IMAGE_MODE_EMBED => ImageMode::Embed,
        _ => return Err(FfiError::invalid_config("unknown image mode")),
    };
    if raw.options & LITEPARSE_CONFIG_FLAG_HAS_CROP_BOX != 0 {
        config.crop_box = Some(crop_box(raw.crop_box)?);
    }

    unsafe {
        if let Some(v) = optional_view_str(raw.ocr_language, "ocr_language")? {
            config.ocr_language = v;
        }
        config.ocr_server_url = optional_view_str(raw.ocr_server_url, "ocr_server_url")?;
        config.tessdata_path = optional_view_str(raw.tessdata_path, "tessdata_path")?;
        config.password = optional_view_str(raw.password, "password")?;
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

    let corrections = unsafe {
        as_slice(
            raw.orientation_corrections,
            raw.orientation_corrections_len,
            "orientation_corrections",
        )
    }?;
    for correction in corrections.unwrap_or_default() {
        if !matches!(correction.angle, 0 | 90 | 180 | 270) {
            return Err(FfiError::invalid_config(format!(
                "orientation_corrections: page {} has angle {}; expected 0, 90, 180 or 270",
                correction.page, correction.angle,
            )));
        }
        config
            .page_orientation_corrections
            .push(PageOrientationCorrection {
                page: correction.page,
                angle: correction.angle as u16,
            });
    }

    Ok(OwnedParserConfig {
        core: config,
        font_db_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dpi_matches_core() {
        assert_eq!(LITEPARSE_DEFAULT_DPI, liteparse::config::DEFAULT_DPI);
    }
}
