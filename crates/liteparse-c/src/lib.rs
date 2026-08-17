//! Typed C ABI for LiteParse.
//!
//! Rust data structures remain behind opaque handles. Configuration is exposed
//! through a typed options builder, and parse results offer borrowed typed
//! views plus explicit JSON serialization.

#![deny(unsafe_op_in_unsafe_fn)]
// The exported functions share the module-level ownership/safety contract in
// the C README; individual signatures are intentionally kept compact.
#![allow(clippy::missing_safety_doc)]

use liteparse::config::{CropBox, ImageMode, LiteParseConfig, OutputFormat};
use liteparse::ocr::{OcrEngine, OcrOptions, OcrResult};
use liteparse::ocr_merge::PageComplexityStats;
use liteparse::search::{SearchOptions, search_items};
use liteparse::types::{PdfInput, TextItem};
use liteparse::{DEFAULT_PAGE_BATCH_SIZE, ParseResult, ParseSession, ScreenshotResult};
use serde_json::{Map, Value};
use std::any::Any;
use std::ffi::{CStr, CString, c_char, c_void};
use std::future::Future;
use std::marker::{PhantomData, PhantomPinned};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::OnceLock;

/// Fixed-width status code returned by every fallible C API function.
pub type LiteParseStatus = u32;

/// The operation completed successfully.
pub const LITEPARSE_STATUS_OK: LiteParseStatus = 0;
/// A required pointer was null or an input string was not valid UTF-8.
pub const LITEPARSE_STATUS_INVALID_ARGUMENT: LiteParseStatus = 1;
/// An options value was invalid or requested an unsupported C feature.
pub const LITEPARSE_STATUS_INVALID_CONFIG: LiteParseStatus = 2;
/// LiteParse could not open, convert, or parse the document.
pub const LITEPARSE_STATUS_PARSE_ERROR: LiteParseStatus = 3;
/// The parse result could not be serialized to JSON.
pub const LITEPARSE_STATUS_SERIALIZATION_ERROR: LiteParseStatus = 4;
/// The binding could not initialize its asynchronous runtime.
pub const LITEPARSE_STATUS_RUNTIME_ERROR: LiteParseStatus = 5;
/// An unwinding Rust panic was caught before it crossed the C ABI boundary.
/// After a parser operation returns this status, free the handle without
/// reusing it. Process-aborting panics and allocation failures are not caught.
pub const LITEPARSE_STATUS_PANIC: LiteParseStatus = 255;

/// Values accepted by `liteparse_options_set_output_format`.
pub const LITEPARSE_OUTPUT_FORMAT_JSON: u32 = 0;
pub const LITEPARSE_OUTPUT_FORMAT_TEXT: u32 = 1;
pub const LITEPARSE_OUTPUT_FORMAT_MARKDOWN: u32 = 2;

/// Values accepted by `liteparse_options_set_image_mode`.
pub const LITEPARSE_IMAGE_MODE_OFF: u32 = 0;
pub const LITEPARSE_IMAGE_MODE_PLACEHOLDER: u32 = 1;
pub const LITEPARSE_IMAGE_MODE_EMBED: u32 = 2;

/// Pixel formats handed to a `LiteParseOcrRecognizeFn`. RGB is tightly packed
/// 3 bytes per pixel; grayscale is 1 byte per pixel.
pub const LITEPARSE_OCR_PIXEL_FORMAT_RGB: u32 = 0;
pub const LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE: u32 = 1;

// Exhaustiveness guard: a new core variant must be mirrored in the constants
// above, the setter matches below, and the checked-in C header.
#[allow(dead_code)]
fn _core_enum_variants_are_mirrored(format: OutputFormat, mode: ImageMode) {
    match format {
        OutputFormat::Json | OutputFormat::Text | OutputFormat::Markdown => {}
    }
    match mode {
        ImageMode::Off | ImageMode::Placeholder | ImageMode::Embed => {}
    }
}

/// Opaque parser handle. Create it with `liteparse_parser_new_with_options`
/// and destroy it with `liteparse_parser_free`.
pub struct LiteParseParser {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Opaque options handle. Create it with `liteparse_options_new` and destroy
/// it with `liteparse_options_free`.
pub struct LiteParseOptions {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Opaque typed parse-result handle. Create it with one of the
/// `liteparse_parse_*_result` functions and destroy it with
/// `liteparse_result_free`.
pub struct LiteParseResult {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Borrowed bytes returned by typed result getters. The pointer remains valid
/// until the owning `LiteParseResult` is freed. Bytes are UTF-8 where the
/// getter says so, but are not NUL-terminated.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseByteView {
    pub ptr: *const u8,
    pub len: usize,
}

/// A text item view. All string fields are borrowed from the result handle.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseTextItem {
    pub text: LiteParseByteView,
    pub font_name: LiteParseByteView,
    pub link: LiteParseByteView,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub font_size: f32,
    pub has_font_size: bool,
    pub confidence: f32,
    pub has_confidence: bool,
}

/// An extracted embedded image view. Image bytes and string fields are
/// borrowed from the result handle.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseImage {
    pub id: LiteParseByteView,
    pub name: LiteParseByteView,
    pub path: LiteParseByteView,
    pub format: LiteParseByteView,
    pub duplicate_of: LiteParseByteView,
    pub page: u32,
    pub width: u32,
    pub height: u32,
    pub rotation: f32,
    pub bbox_x: f32,
    pub bbox_y: f32,
    pub bbox_width: f32,
    pub bbox_height: f32,
    pub bytes: LiteParseByteView,
}

/// Opaque page-screenshot collection handle. Create it with
/// `liteparse_screenshot_path` or `liteparse_screenshot_bytes` and destroy it
/// with `liteparse_screenshots_free`.
pub struct LiteParseScreenshots {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Opaque per-page complexity report handle. Create it with
/// `liteparse_is_complex_path` or `liteparse_is_complex_bytes` and destroy it
/// with `liteparse_complexity_free`.
pub struct LiteParseComplexity {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Opaque bounded-memory batch-parsing session handle. Create it with
/// `liteparse_session_open_path` or `liteparse_session_open_bytes` and destroy
/// it with `liteparse_session_free`.
pub struct LiteParseSession {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// One rendered page screenshot. PNG bytes are borrowed from the owning
/// `LiteParseScreenshots` handle.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseScreenshot {
    /// 1-based source page number.
    pub page_number: u32,
    /// Rendered bitmap width in pixels.
    pub width: u32,
    /// Rendered bitmap height in pixels.
    pub height: u32,
    /// True when every pixel has the same color (blank page after render).
    pub is_solid_fill: bool,
    /// PNG-encoded image bytes.
    pub png: LiteParseByteView,
    /// Number of rects readable via `liteparse_screenshots_get_rect`.
    /// Non-zero only when `detect_screenshot_rects` was enabled.
    pub rect_count: usize,
}

/// One solid rectangle (or thick line) detected in a rendered page bitmap.
/// Coordinates are in top-left-origin 72-DPI viewport space. The color is a
/// borrowed UTF-8 ARGB hex string (e.g. "ff1a2b3c").
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseScreenshotRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// True when only one dimension reaches the minimum rectangle size
    /// (a solid line rather than a filled area).
    pub is_line: bool,
    pub color: LiteParseByteView,
}

/// Flat per-page complexity signals. The flag reasons and layout signals are
/// available through `liteparse_complexity_to_json`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParsePageComplexity {
    /// 1-based source page number.
    pub page_number: usize,
    /// Usable native text length in bytes.
    pub text_length: usize,
    /// Fraction of the page area covered by native text.
    pub text_coverage: f32,
    /// Number of raster image objects counted on the page.
    pub image_block_count: usize,
    /// Summed image-bbox area over the page area, clamped to 1.0.
    pub image_coverage: f32,
    /// Area of the single largest counted image over the page area.
    pub largest_image_coverage: f32,
    pub has_substantial_images: bool,
    /// A single raster covering >= 90% of the page is present (scan signal).
    pub full_page_image: bool,
    pub is_garbled: bool,
    /// Whether the page needs more than the cheap text-only path.
    pub needs_ocr: bool,
    /// Page area in pt^2.
    pub page_area: f32,
    /// Filled vector-outline area not covered by native text, in pt^2.
    /// Meaningful only when `has_uncovered_vector_area` is true.
    pub uncovered_vector_area: f32,
    pub has_uncovered_vector_area: bool,
}

/// Opaque phrase-search match collection handle. Create it with
/// `liteparse_result_search` and destroy it with
/// `liteparse_search_matches_free`.
pub struct LiteParseSearchMatches {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// One word's bounding box within a text item, in the same top-left-origin
/// 72-DPI viewport space as the parent item. `text` is borrowed from the
/// owning `LiteParseResult` and excludes inter-word spaces.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteParseWordBox {
    pub text: LiteParseByteView,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Opaque OCR result sink passed to a `LiteParseOcrRecognizeFn`; valid only
/// during that invocation and never retained.
pub struct LiteParseOcrSink {
    _private: [u8; 0],
    _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// A custom in-process OCR engine, called once per rendered page raster.
///
/// `pixels` holds `pixels_len` bytes of tightly packed image data in the
/// given `pixel_format` (`width * height` bytes for grayscale, `* 3` for
/// RGB); `language` and `dpi` come from the parser configuration. Push
/// word-level results into `sink` with `liteparse_ocr_sink_add` and return
/// zero. A non-zero return fails the page's OCR with the message set via
/// `liteparse_ocr_sink_set_error`.
///
/// The callback must not unwind, must not retain any argument past the call,
/// and must be thread-safe: parses can invoke it from several threads at
/// once. Null is a valid value and clears a registration.
pub type LiteParseOcrRecognizeFn = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        pixels: *const u8,
        pixels_len: usize,
        width: u32,
        height: u32,
        pixel_format: u32,
        language: *const c_char,
        dpi: f32,
        sink: *mut LiteParseOcrSink,
    ) -> u32,
>;

/// Non-null inner type of [`LiteParseOcrRecognizeFn`]; internal storage
/// only, not part of the header.
type OcrRecognizeRaw = unsafe extern "C" fn(
    user_data: *mut c_void,
    pixels: *const u8,
    pixels_len: usize,
    width: u32,
    height: u32,
    pixel_format: u32,
    language: *const c_char,
    dpi: f32,
    sink: *mut LiteParseOcrSink,
) -> u32;

/// Raw `user_data` moved across threads; the C callback contract makes the
/// caller responsible for thread safety.
#[derive(Clone, Copy)]
struct OcrUserData(*mut c_void);
unsafe impl Send for OcrUserData {}
unsafe impl Sync for OcrUserData {}

/// The C callback registration captured on an options handle.
#[derive(Clone)]
struct CallbackOcrSpec {
    recognize: OcrRecognizeRaw,
    user_data: OcrUserData,
    name: String,
    prefers_grayscale: bool,
}

/// Bridges a registered C callback into the core `OcrEngine` trait.
struct CallbackOcrEngine {
    spec: CallbackOcrSpec,
}

struct OcrSinkState {
    results: Vec<OcrResult>,
    error: Option<String>,
}

impl OcrEngine for CallbackOcrEngine {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn prefers_grayscale(&self) -> bool {
        self.spec.prefers_grayscale
    }

    fn recognize<'a, 'b: 'a, 'c: 'a>(
        &'a self,
        image_data: &'c [u8],
        width: u32,
        height: u32,
        options: &'b OcrOptions,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = Result<Vec<OcrResult>, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + '_,
        >,
    > {
        // The C callback is synchronous; the future is just its ready result.
        let result = (|| {
            let language = CString::new(options.language.as_str())
                .map_err(|_| "ocr_language contains an interior NUL byte".to_string())?;
            let pixel_format = if self.spec.prefers_grayscale {
                LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE
            } else {
                LITEPARSE_OCR_PIXEL_FORMAT_RGB
            };
            let mut sink = OcrSinkState {
                results: Vec::new(),
                error: None,
            };
            // SAFETY: The registration contract guarantees the callback and
            // `user_data` stay valid and thread-safe for the parser's life.
            // All pointers passed here outlive the synchronous call.
            let status = unsafe {
                (self.spec.recognize)(
                    self.spec.user_data.0,
                    image_data.as_ptr(),
                    image_data.len(),
                    width,
                    height,
                    pixel_format,
                    language.as_ptr(),
                    options.dpi,
                    ptr::from_mut(&mut sink).cast::<LiteParseOcrSink>(),
                )
            };
            if status != 0 {
                return Err(sink
                    .error
                    .unwrap_or_else(|| format!("OCR callback failed with status {status}")));
            }
            Ok(sink.results)
        })();
        Box::pin(std::future::ready(result.map_err(Into::into)))
    }
}

struct ParserState {
    parser: liteparse::LiteParse,
}

struct SearchState {
    items: Vec<TextItem>,
}

struct ScreenshotsState {
    screenshots: Vec<ScreenshotResult>,
}

struct ComplexityState {
    stats: Vec<PageComplexityStats>,
}

struct SessionState {
    session: ParseSession,
    extract_text_metadata: bool,
}

struct OptionsState {
    config: LiteParseConfig,
    ocr_callback: Option<CallbackOcrSpec>,
}

struct ResultState {
    result: ParseResult,
    extract_text_metadata: bool,
}

const fn assert_send_sync<T: Send + Sync>() {}
const _: () = assert_send_sync::<ParserState>();
// Sessions are single-owner (`next_batch` mutates), so only Send is required
// to let the caller move the handle between threads.
const fn assert_send<T: Send>() {}
const _: () = assert_send::<SessionState>();

#[derive(Debug)]
struct FfiError {
    status: LiteParseStatus,
    message: String,
}

impl FfiError {
    fn new(status: LiteParseStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(LITEPARSE_STATUS_INVALID_ARGUMENT, message)
    }

    fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(LITEPARSE_STATUS_INVALID_CONFIG, message)
    }

    fn serialization(error: impl ToString) -> Self {
        Self::new(LITEPARSE_STATUS_SERIALIZATION_ERROR, error.to_string())
    }
}

type FfiResult<T = ()> = Result<T, FfiError>;

/// Associates each opaque handle type with its state type, making a
/// mismatched handle/state pairing a compile error.
trait Opaque {
    type State;
    /// The parameter name used in null-handle error messages.
    const NAME: &'static str;
}

impl Opaque for LiteParseOptions {
    type State = OptionsState;
    const NAME: &'static str = "options";
}
impl Opaque for LiteParseParser {
    type State = ParserState;
    const NAME: &'static str = "parser";
}
impl Opaque for LiteParseResult {
    type State = ResultState;
    const NAME: &'static str = "result";
}
impl Opaque for LiteParseScreenshots {
    type State = ScreenshotsState;
    const NAME: &'static str = "screenshots";
}
impl Opaque for LiteParseComplexity {
    type State = ComplexityState;
    const NAME: &'static str = "complexity";
}
impl Opaque for LiteParseSearchMatches {
    type State = SearchState;
    const NAME: &'static str = "matches";
}
impl Opaque for LiteParseSession {
    type State = SessionState;
    const NAME: &'static str = "session";
}
impl Opaque for LiteParseOcrSink {
    type State = OcrSinkState;
    const NAME: &'static str = "sink";
}

/// Borrow a handle's state for the duration of the caller's FFI operation.
unsafe fn state_ref<'a, H: Opaque>(handle: *const H) -> FfiResult<&'a H::State> {
    // SAFETY: The caller contract requires a live handle of type `H`.
    unsafe { handle.cast::<H::State>().as_ref() }
        .ok_or_else(|| FfiError::invalid_argument(format!("{} must not be null", H::NAME)))
}

/// Mutably borrow a single-owner handle's state (options, sessions, the OCR
/// sink); the handle must not be aliased while the borrow is live.
unsafe fn state_mut<'a, H: Opaque>(handle: *mut H) -> FfiResult<&'a mut H::State> {
    // SAFETY: The caller contract requires a live, unaliased handle.
    unsafe { handle.cast::<H::State>().as_mut() }
        .ok_or_else(|| FfiError::invalid_argument(format!("{} must not be null", H::NAME)))
}

fn validate_config_semantics(config: &LiteParseConfig) -> FfiResult {
    if config.image_output_dir.is_some() && !config.effective_extract_images() {
        return Err(FfiError::invalid_config(
            "image_output_dir requires extract_images = true or image_mode = \"embed\"",
        ));
    }
    Ok(())
}

unsafe fn set_option<F>(
    options: *mut LiteParseOptions,
    out_error: *mut *mut c_char,
    update: F,
) -> LiteParseStatus
where
    F: FnOnce(&mut LiteParseConfig) -> FfiResult,
{
    ffi_boundary(out_error, || {
        // SAFETY: The caller supplies a live options handle by contract.
        let state = unsafe { state_mut(options) }?;
        // Updaters validate their value before mutating, so a failed setter
        // leaves the config unchanged. Cross-field rules are checked once in
        // `liteparse_parser_new_with_options`.
        update(&mut state.config)
    })
}

/// Create options initialized with LiteParse defaults.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_new(
    out_options: *mut *mut LiteParseOptions,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_options, "out_options")?;
        emit_handle(out_options, || {
            Ok(OptionsState {
                config: LiteParseConfig::default(),
                ocr_callback: None,
            })
        })
    })
}

/// Destroy an options handle. A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_free(options: *mut LiteParseOptions) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(options) };
}

// The per-field setters below are written out longhand rather than
// macro-generated: cbindgen only sees macro-expanded items with nightly
// `parse.expand`, and the checked-in header must stay regenerable.
unsafe fn set_bool_option(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
    update: impl FnOnce(&mut LiteParseConfig, bool),
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |config| {
            update(config, value);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_continue_on_page_error(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.continue_on_page_error = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_detect_screenshot_rects(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.detect_screenshot_rects = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_emit_word_boxes(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.emit_word_boxes = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_annotations(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_annotations = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_blocks(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_blocks = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_content_bounds(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.extract_content_bounds = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_document_metadata(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.extract_document_metadata = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_form_fields(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_form_fields = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_images(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_images = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_links(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_links = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_structure_tree(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.extract_structure_tree = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_text_metadata(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.extract_text_metadata = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_vector_graphics(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.extract_vector_graphics = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_extract_xfa_packets(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.extract_xfa_packets = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_include_complexity(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.include_complexity = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_keep_headers_footers(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.keep_headers_footers = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_ocr_enabled(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.ocr_enabled = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_ocr_failure_fatal(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.ocr_failure_fatal = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_preserve_very_small_text(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_bool_option(options, value, out_error, |c, v| {
            c.preserve_very_small_text = v
        })
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_quiet(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.quiet = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_render_form_fields(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.render_form_fields = v) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_skip_diagonal_text(
    options: *mut LiteParseOptions,
    value: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe { set_bool_option(options, value, out_error, |c, v| c.skip_diagonal_text = v) }
}

unsafe fn set_optional_string_option(
    options: *mut LiteParseOptions,
    value: *const c_char,
    name: &'static str,
    out_error: *mut *mut c_char,
    update: impl FnOnce(&mut LiteParseConfig, Option<String>),
) -> LiteParseStatus {
    // SAFETY: `read_utf8` is called only for a non-null pointer supplied by the
    // caller, and `set_option` owns the mutable config borrow for this call.
    unsafe {
        set_option(options, out_error, |config| {
            let value = if value.is_null() {
                None
            } else {
                Some(read_utf8(value, name)?)
            };
            update(config, value);
            Ok(())
        })
    }
}

unsafe fn set_required_string_option(
    options: *mut LiteParseOptions,
    value: *const c_char,
    name: &'static str,
    out_error: *mut *mut c_char,
    update: impl FnOnce(&mut LiteParseConfig, String),
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |config| {
            let value = read_utf8(value, name)?;
            update(config, value);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_ocr_language(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    // SAFETY: The helper validates the pointer and UTF-8 input.
    unsafe {
        set_required_string_option(options, value, "ocr_language", out_error, |c, v| {
            c.ocr_language = v
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_ocr_server_url(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_optional_string_option(options, value, "ocr_server_url", out_error, |c, v| {
            c.ocr_server_url = v
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_tessdata_path(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_optional_string_option(options, value, "tessdata_path", out_error, |c, v| {
            c.tessdata_path = v
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_target_pages(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_optional_string_option(options, value, "target_pages", out_error, |c, v| {
            c.target_pages = v
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_password(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_optional_string_option(options, value, "password", out_error, |c, v| c.password = v)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_image_output_dir(
    options: *mut LiteParseOptions,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_optional_string_option(options, value, "image_output_dir", out_error, |c, v| {
            c.image_output_dir = v
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_max_pages(
    options: *mut LiteParseOptions,
    value: usize,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.max_pages = value;
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_num_workers(
    options: *mut LiteParseOptions,
    value: usize,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            if value == 0 {
                return Err(FfiError::invalid_config(
                    "num_workers must be greater than zero",
                ));
            }
            c.num_workers = value;
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_dpi(
    options: *mut LiteParseOptions,
    value: f32,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            if !value.is_finite() || value <= 0.0 {
                return Err(FfiError::invalid_config(
                    "dpi must be finite and greater than zero",
                ));
            }
            c.dpi = value;
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_output_format(
    options: *mut LiteParseOptions,
    value: u32,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.output_format = match value {
                LITEPARSE_OUTPUT_FORMAT_JSON => OutputFormat::Json,
                LITEPARSE_OUTPUT_FORMAT_TEXT => OutputFormat::Text,
                LITEPARSE_OUTPUT_FORMAT_MARKDOWN => OutputFormat::Markdown,
                _ => return Err(FfiError::invalid_config("unknown output format")),
            };
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_image_mode(
    options: *mut LiteParseOptions,
    value: u32,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.image_mode = match value {
                LITEPARSE_IMAGE_MODE_OFF => ImageMode::Off,
                LITEPARSE_IMAGE_MODE_PLACEHOLDER => ImageMode::Placeholder,
                LITEPARSE_IMAGE_MODE_EMBED => ImageMode::Embed,
                _ => return Err(FfiError::invalid_config("unknown image mode")),
            };
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_crop_box(
    options: *mut LiteParseOptions,
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            let values = [top, right, bottom, left];
            if values
                .iter()
                .any(|v| !v.is_finite() || *v < 0.0 || *v > 1.0)
                || top + bottom >= 1.0
                || left + right >= 1.0
            {
                return Err(FfiError::invalid_config(
                    "crop_box values must be finite fractions with each opposing pair below one",
                ));
            }
            c.crop_box = Some(CropBox {
                top,
                right,
                bottom,
                left,
            });
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_clear_crop_box(
    options: *mut LiteParseOptions,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.crop_box = None;
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_add_ocr_server_header(
    options: *mut LiteParseOptions,
    name: *const c_char,
    value: *const c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            let name = read_utf8(name, "header name")?;
            let value = read_utf8(value, "header value")?;
            c.ocr_server_headers.push((name, value));
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_clear_ocr_server_headers(
    options: *mut LiteParseOptions,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.ocr_server_headers.clear();
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_add_ocr_hedge_delay_ms(
    options: *mut LiteParseOptions,
    delay_ms: u64,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.ocr_hedge_delays_ms.push(delay_ms);
            Ok(())
        })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_clear_ocr_hedge_delays(
    options: *mut LiteParseOptions,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        set_option(options, out_error, |c| {
            c.ocr_hedge_delays_ms.clear();
            Ok(())
        })
    }
}

/// Register a custom in-process OCR engine on an options handle.
///
/// Parsers created from these options call `recognize` instead of the
/// built-in Tesseract or HTTP engines; OCR still requires `ocr_enabled`.
/// `user_data` is passed through verbatim, `name` labels the engine in logs
/// (null selects "c-callback"), and `prefers_grayscale` picks the raster
/// format. A null `recognize` clears the registration.
///
/// # Safety
///
/// `name`, when non-null, must be NUL-terminated. The callback and
/// `user_data` must stay valid, and thread-safe, for the lifetime of every
/// parser created from these options.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_options_set_ocr_callback(
    options: *mut LiteParseOptions,
    recognize: LiteParseOcrRecognizeFn,
    user_data: *mut c_void,
    name: *const c_char,
    prefers_grayscale: bool,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        let state = state_mut(options)?;
        let Some(recognize) = recognize else {
            state.ocr_callback = None;
            return Ok(());
        };
        let name = if name.is_null() {
            "c-callback".to_owned()
        } else {
            read_utf8(name, "name")?
        };
        state.ocr_callback = Some(CallbackOcrSpec {
            recognize,
            user_data: OcrUserData(user_data),
            name,
            prefers_grayscale,
        });
        Ok(())
    })
}

/// Append one word-level OCR result from inside a `LiteParseOcrRecognizeFn`.
///
/// The box is `[x1, y1, x2, y2]` (left, top, right, bottom) in pixel
/// coordinates of the callback's raster; `confidence` is 0.0-1.0. `polygon`
/// is null, or the 8 x/y floats of a rotated detection quad (top-left,
/// top-right, bottom-right, bottom-left in reading order) so rotated text
/// orientation can be recovered.
///
/// # Safety
///
/// `sink` must be the current invocation's live sink, `text` NUL-terminated
/// UTF-8, and `polygon` null or 8 readable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_add(
    sink: *mut LiteParseOcrSink,
    text: *const c_char,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    confidence: f32,
    polygon: *const f32,
) -> LiteParseStatus {
    catch_status(|| unsafe {
        let state = state_mut(sink)?;
        let text = read_utf8(text, "text")?;
        let polygon = if polygon.is_null() {
            None
        } else {
            // SAFETY: The caller guarantees 8 readable floats.
            let p = std::slice::from_raw_parts(polygon, 8);
            Some([[p[0], p[1]], [p[2], p[3]], [p[4], p[5]], [p[6], p[7]]])
        };
        state.results.push(OcrResult {
            text,
            bbox: [x1, y1, x2, y2],
            confidence,
            polygon,
        });
        Ok(())
    })
}

/// Set the failure message reported when the current callback invocation
/// returns non-zero; ignored on success. Later calls replace earlier ones.
///
/// # Safety
///
/// `sink` must be the current invocation's live sink and `message` a
/// NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_set_error(
    sink: *mut LiteParseOcrSink,
    message: *const c_char,
) -> LiteParseStatus {
    catch_status(|| unsafe {
        let state = state_mut(sink)?;
        state.error = Some(read_utf8(message, "message")?);
        Ok(())
    })
}

/// Return the LiteParse C binding version as a borrowed, NUL-terminated UTF-8
/// string. The returned pointer is static and must not be freed.
#[unsafe(no_mangle)]
pub extern "C" fn liteparse_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Create a parser from a typed options handle. The options remain owned by
/// the caller and may be reused to create additional parsers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_new_with_options(
    options: *const LiteParseOptions,
    out_parser: *mut *mut LiteParseParser,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_parser, "out_parser")?;
        let options = state_ref(options)?;
        validate_config_semantics(&options.config)?;
        emit_handle(out_parser, || {
            build_parser_state(options.config.clone(), options.ocr_callback.clone())
        })
    })
}

/// Destroy a parser handle. A null pointer is accepted as a no-op. Must not
/// race with an active call on the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_free(parser: *mut LiteParseParser) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(parser) };
}

/// Parse a document from a filesystem path and return structured JSON: the
/// CLI JSON schema plus top-level `text`, `creator`, `producer`, and
/// `doc_meta` fields.
///
/// On success `*out_json` receives an owned NUL-terminated UTF-8 string; on
/// failure it stays null and `*out_error` (when non-null) receives an owned
/// error string. Free either with `liteparse_string_free`.
///
/// # Safety
///
/// `parser` must be a live handle, `path` NUL-terminated, and each output
/// slot valid for one non-overlapping pointer write (`out_error` may be
/// null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parse_path(
    parser: *const LiteParseParser,
    path: *const c_char,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_json, "out_json")?;
        let path = read_utf8(path, "path")?;
        let (result, extract_text_metadata) = run_parse(parser, PdfInput::Path(path))?;
        let json = format_result(&result, extract_text_metadata)?;
        write_string(out_json, json)
    })
}

/// Parse a document from an in-memory byte buffer and return structured
/// JSON. The input is copied, so the caller may free its buffer on return;
/// ownership matches `liteparse_parse_path`.
///
/// # Safety
///
/// As `liteparse_parse_path`, except `data` must be readable for `data_len`
/// bytes (null only with `data_len` zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parse_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_json, "out_json")?;
        let bytes = copy_input_bytes(data, data_len)?;
        let (result, extract_text_metadata) = run_parse(parser, PdfInput::Bytes(bytes))?;
        let json = format_result(&result, extract_text_metadata)?;
        write_string(out_json, json)
    })
}

/// Parse a document into a typed result handle. The handle exposes page and
/// text-item getters and remains independent of the JSON convenience API.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parse_path_result(
    parser: *const LiteParseParser,
    path: *const c_char,
    out_result: *mut *mut LiteParseResult,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_result, "out_result")?;
        let path = read_utf8(path, "path")?;
        let (result, extract_text_metadata) = run_parse(parser, PdfInput::Path(path))?;
        emit_handle(out_result, || {
            Ok(ResultState {
                result,
                extract_text_metadata,
            })
        })
    })
}

/// Parse an in-memory document into a typed result handle. The input bytes are
/// copied before parsing and may be released when this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parse_bytes_result(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    out_result: *mut *mut LiteParseResult,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_result, "out_result")?;
        let bytes = copy_input_bytes(data, data_len)?;
        let (result, extract_text_metadata) = run_parse(parser, PdfInput::Bytes(bytes))?;
        emit_handle(out_result, || {
            Ok(ResultState {
                result,
                extract_text_metadata,
            })
        })
    })
}

/// Destroy a typed result handle. A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_free(result: *mut LiteParseResult) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(result) };
}

/// Serialize a typed result to the same pretty JSON representation returned by
/// `liteparse_parse_path` and `liteparse_parse_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_to_json(
    result: *const LiteParseResult,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_json, "out_json")?;
        let result = state_ref(result)?;
        let json = format_result(&result.result, result.extract_text_metadata)?;
        write_string(out_json, json)
    })
}

/// Return the source document page count, or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_total_pages(result: *const LiteParseResult) -> u32 {
    // SAFETY: The getter is total for null pointers and invalid indexes.
    unsafe { state_ref(result).ok().map_or(0, |r| r.result.total_pages) }
}

/// Return the number of parsed pages, or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_page_count(result: *const LiteParseResult) -> usize {
    unsafe { state_ref(result).ok().map_or(0, |r| r.result.pages.len()) }
}

/// Return a page's 1-based source page number, or zero for an invalid index.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_page_number(
    result: *const LiteParseResult,
    page_index: usize,
) -> u32 {
    unsafe {
        state_ref(result)
            .ok()
            .and_then(|r| r.result.pages.get(page_index))
            .map_or(0, |page| page.page_number.min(u32::MAX as usize) as u32)
    }
}

/// Return the full-document UTF-8 text as a borrowed byte view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_text(
    result: *const LiteParseResult,
    out: *mut LiteParseByteView,
) -> bool {
    unsafe {
        write_out(
            out,
            (state_ref(result).ok().map(|r| r.result.text.as_str()))
                .map(|v| bytes_view(v.as_bytes())),
        )
    }
}

/// Return one parsed page's UTF-8 text as a borrowed byte view.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_page_text(
    result: *const LiteParseResult,
    page_index: usize,
    out: *mut LiteParseByteView,
) -> bool {
    unsafe {
        write_out(
            out,
            (state_ref(result)
                .ok()
                .and_then(|r| r.result.pages.get(page_index))
                .map(|page| page.text.as_str()))
            .map(|v| bytes_view(v.as_bytes())),
        )
    }
}

/// Return the document's optional `/Info` Creator value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_creator(
    result: *const LiteParseResult,
    out: *mut LiteParseByteView,
) -> bool {
    unsafe {
        write_out(
            out,
            (state_ref(result)
                .ok()
                .and_then(|r| r.result.creator.as_deref()))
            .map(|v| bytes_view(v.as_bytes())),
        )
    }
}

/// Return the document's optional `/Info` Producer value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_producer(
    result: *const LiteParseResult,
    out: *mut LiteParseByteView,
) -> bool {
    unsafe {
        write_out(
            out,
            (state_ref(result)
                .ok()
                .and_then(|r| r.result.producer.as_deref()))
            .map(|v| bytes_view(v.as_bytes())),
        )
    }
}

/// Return one page's viewport dimensions in 72-DPI points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_page_size(
    result: *const LiteParseResult,
    page_index: usize,
    out_width: *mut f32,
    out_height: *mut f32,
) -> bool {
    unsafe {
        let page = state_ref(result)
            .ok()
            .and_then(|r| r.result.pages.get(page_index));
        // Both writes must go through so a partial-null call still clears the
        // writable slot; hence two write_out calls rather than a short-circuit.
        let wrote_width = write_out(out_width, page.map(|p| p.page_width));
        let wrote_height = write_out(out_height, page.map(|p| p.page_height));
        wrote_width && wrote_height
    }
}

/// Return the number of text items on one parsed page.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_text_item_count(
    result: *const LiteParseResult,
    page_index: usize,
) -> usize {
    unsafe {
        state_ref(result)
            .ok()
            .and_then(|r| r.result.pages.get(page_index))
            .map_or(0, |page| page.text_items.len())
    }
}

/// Return one text item and its borrowed UTF-8 fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_text_item(
    result: *const LiteParseResult,
    page_index: usize,
    item_index: usize,
    out: *mut LiteParseTextItem,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(result)
                .ok()
                .and_then(|r| r.result.pages.get(page_index))
                .and_then(|page| page.text_items.get(item_index))
                .map(text_item_view),
        )
    }
}

/// Return the number of word boxes on one text item. Always zero unless
/// `emit_word_boxes` was enabled in the options builder.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_word_box_count(
    result: *const LiteParseResult,
    page_index: usize,
    item_index: usize,
) -> usize {
    unsafe {
        state_ref(result)
            .ok()
            .and_then(|r| r.result.pages.get(page_index))
            .and_then(|page| page.text_items.get(item_index))
            .map_or(0, |item| item.words.len())
    }
}

/// Return one word box of one text item. Word boxes are absent from the JSON
/// output; this getter is their only access path. Populated only when
/// `emit_word_boxes` was enabled and the item produced a word split.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_word_box(
    result: *const LiteParseResult,
    page_index: usize,
    item_index: usize,
    word_index: usize,
    out: *mut LiteParseWordBox,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(result)
                .ok()
                .and_then(|r| r.result.pages.get(page_index))
                .and_then(|page| page.text_items.get(item_index))
                .and_then(|item| item.words.get(word_index))
                .map(|word| LiteParseWordBox {
                    text: byte_view(Some(word.text.as_str())),
                    x: word.x,
                    y: word.y,
                    width: word.width,
                    height: word.height,
                }),
        )
    }
}

/// Return the number of extracted embedded images.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_image_count(result: *const LiteParseResult) -> usize {
    unsafe { state_ref(result).ok().map_or(0, |r| r.result.images.len()) }
}

/// Return one extracted embedded image and its borrowed metadata/bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_get_image(
    result: *const LiteParseResult,
    image_index: usize,
    out: *mut LiteParseImage,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(result)
                .ok()
                .and_then(|r| r.result.images.get(image_index))
                .map(|image| LiteParseImage {
                    id: byte_view(Some(image.id.as_str())),
                    name: byte_view(Some(image.name.as_str())),
                    path: byte_view(image.path.as_deref()),
                    format: byte_view(Some(image.format.as_str())),
                    duplicate_of: byte_view(image.duplicate_of.as_deref()),
                    page: image.page,
                    width: image.width,
                    height: image.height,
                    rotation: image.rotation,
                    bbox_x: image.bbox.x,
                    bbox_y: image.bbox.y,
                    bbox_width: image.bbox.width,
                    bbox_height: image.bbox.height,
                    bytes: bytes_view(image.bytes.as_slice()),
                }),
        )
    }
}

/// Search one parsed page's text items for phrase matches.
///
/// Consecutive items are concatenated, so a spanning phrase yields one
/// merged item with a combined bounding box. An empty phrase matches
/// nothing; an out-of-range `page_index` is an invalid argument. Matches are
/// copies, independent of `result`.
///
/// # Safety
///
/// `result` must be a live handle, `phrase` NUL-terminated, and each output
/// slot valid for one non-overlapping pointer write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_result_search(
    result: *const LiteParseResult,
    page_index: usize,
    phrase: *const c_char,
    case_sensitive: bool,
    out_matches: *mut *mut LiteParseSearchMatches,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_matches, "out_matches")?;
        let phrase = read_utf8(phrase, "phrase")?;
        let state = state_ref(result)?;
        let Some(page) = state.result.pages.get(page_index) else {
            return Err(FfiError::invalid_argument(format!(
                "page_index {page_index} is out of range for {} parsed pages",
                state.result.pages.len()
            )));
        };
        let options = SearchOptions {
            phrase,
            case_sensitive,
        };
        let items = search_items(&page.text_items, &options);
        emit_handle(out_matches, || Ok(SearchState { items }))
    })
}

/// Destroy a search-match handle. A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_free(matches: *mut LiteParseSearchMatches) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(matches) };
}

/// Return the number of phrase matches, or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_get_count(
    matches: *const LiteParseSearchMatches,
) -> usize {
    unsafe { state_ref(matches).ok().map_or(0, |state| state.items.len()) }
}

/// Return one phrase match as a text-item view. String fields are borrowed
/// from the owning `LiteParseSearchMatches` handle, not the parse result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_search_matches_get(
    matches: *const LiteParseSearchMatches,
    index: usize,
    out: *mut LiteParseTextItem,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(matches)
                .ok()
                .and_then(|state| state.items.get(index))
                .map(text_item_view),
        )
    }
}

/// Render document pages to PNG screenshots.
///
/// `page_numbers` selects 1-based source pages; null with length zero
/// renders every page. Non-PDF inputs are converted first. Rect detection is
/// controlled by `liteparse_options_set_detect_screenshot_rects`.
///
/// # Safety
///
/// As `liteparse_parse_path`, plus `page_numbers` must be readable for
/// `page_numbers_len` values (null only with length zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshot_path(
    parser: *const LiteParseParser,
    path: *const c_char,
    page_numbers: *const u32,
    page_numbers_len: usize,
    out_screenshots: *mut *mut LiteParseScreenshots,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_screenshots, "out_screenshots")?;
        let path = read_utf8(path, "path")?;
        let pages = copy_page_numbers(page_numbers, page_numbers_len)?;
        let screenshots = run_screenshot(parser, PdfInput::Path(path), pages)?;
        emit_handle(out_screenshots, || Ok(ScreenshotsState { screenshots }))
    })
}

/// Render pages of an in-memory document to PNG screenshots. The input bytes
/// are copied before rendering. See `liteparse_screenshot_path` for the page
/// selection and safety contract; `data` follows `liteparse_parse_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshot_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    page_numbers: *const u32,
    page_numbers_len: usize,
    out_screenshots: *mut *mut LiteParseScreenshots,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_screenshots, "out_screenshots")?;
        let bytes = copy_input_bytes(data, data_len)?;
        let pages = copy_page_numbers(page_numbers, page_numbers_len)?;
        let screenshots = run_screenshot(parser, PdfInput::Bytes(bytes), pages)?;
        emit_handle(out_screenshots, || Ok(ScreenshotsState { screenshots }))
    })
}

/// Destroy a screenshots handle. A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_free(screenshots: *mut LiteParseScreenshots) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(screenshots) };
}

/// Return the number of rendered pages, or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_get_count(
    screenshots: *const LiteParseScreenshots,
) -> usize {
    unsafe {
        state_ref(screenshots)
            .ok()
            .map_or(0, |s| s.screenshots.len())
    }
}

/// Return one rendered page and its borrowed PNG bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_get(
    screenshots: *const LiteParseScreenshots,
    index: usize,
    out: *mut LiteParseScreenshot,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(screenshots)
                .ok()
                .and_then(|s| s.screenshots.get(index))
                .map(|screenshot| LiteParseScreenshot {
                    page_number: screenshot.page_num,
                    width: screenshot.width,
                    height: screenshot.height,
                    is_solid_fill: screenshot.is_solid_fill,
                    png: bytes_view(screenshot.image_bytes.as_slice()),
                    rect_count: screenshot.rects.len(),
                }),
        )
    }
}

/// Return one detected rect of one rendered page.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_screenshots_get_rect(
    screenshots: *const LiteParseScreenshots,
    index: usize,
    rect_index: usize,
    out: *mut LiteParseScreenshotRect,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(screenshots)
                .ok()
                .and_then(|s| s.screenshots.get(index))
                .and_then(|screenshot| screenshot.rects.get(rect_index))
                .map(|rect| LiteParseScreenshotRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                    is_line: rect.is_line,
                    color: byte_view(Some(rect.color.as_str())),
                }),
        )
    }
}

/// Compute per-page complexity signals, a cheap pre-OCR check for deciding
/// whether a document needs advanced parsing. Pages dropped by
/// `continue_on_page_error` are absent, so match entries to source pages by
/// `page_number`, not by index.
///
/// # Safety
///
/// Same contract as `liteparse_parse_path`, with `out_complexity` in place of
/// `out_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_is_complex_path(
    parser: *const LiteParseParser,
    path: *const c_char,
    out_complexity: *mut *mut LiteParseComplexity,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_complexity, "out_complexity")?;
        let path = read_utf8(path, "path")?;
        let stats = run_is_complex(parser, PdfInput::Path(path))?;
        emit_handle(out_complexity, || Ok(ComplexityState { stats }))
    })
}

/// Compute per-page complexity signals for an in-memory document. The input
/// bytes are copied before parsing; `data` follows `liteparse_parse_bytes`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_is_complex_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    out_complexity: *mut *mut LiteParseComplexity,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_complexity, "out_complexity")?;
        let bytes = copy_input_bytes(data, data_len)?;
        let stats = run_is_complex(parser, PdfInput::Bytes(bytes))?;
        emit_handle(out_complexity, || Ok(ComplexityState { stats }))
    })
}

/// Destroy a complexity handle. A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_free(complexity: *mut LiteParseComplexity) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(complexity) };
}

/// Return the number of analyzed pages, or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_get_count(
    complexity: *const LiteParseComplexity,
) -> usize {
    unsafe { state_ref(complexity).ok().map_or(0, |c| c.stats.len()) }
}

/// Return one page's flat complexity signals.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_get(
    complexity: *const LiteParseComplexity,
    index: usize,
    out: *mut LiteParsePageComplexity,
) -> bool {
    unsafe {
        write_out(
            out,
            state_ref(complexity)
                .ok()
                .and_then(|c| c.stats.get(index))
                .map(|stats| LiteParsePageComplexity {
                    page_number: stats.page_number,
                    text_length: stats.text_length,
                    text_coverage: stats.text_coverage,
                    image_block_count: stats.image_block_count,
                    image_coverage: stats.image_coverage,
                    largest_image_coverage: stats.largest_image_coverage,
                    has_substantial_images: stats.has_substantial_images,
                    full_page_image: stats.full_page_image,
                    is_garbled: stats.is_garbled,
                    needs_ocr: stats.needs_ocr,
                    page_area: stats.page_area,
                    uncovered_vector_area: stats.uncovered_vector_area.unwrap_or_default(),
                    has_uncovered_vector_area: stats.uncovered_vector_area.is_some(),
                }),
        )
    }
}

/// Serialize the full complexity report, including the per-page flag reasons
/// and layout signals absent from the flat struct, as a pretty JSON array.
/// String ownership matches `liteparse_parse_path`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_complexity_to_json(
    complexity: *const LiteParseComplexity,
    out_json: *mut *mut c_char,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_json, "out_json")?;
        let state = state_ref(complexity)?;
        let json = serde_json::to_string_pretty(&state.stats).map_err(FfiError::serialization)?;
        write_string(out_json, json)
    })
}

/// Open a document for bounded-memory batch parsing.
///
/// The session snapshots the parser's configuration and may outlive the
/// parser handle. `liteparse_session_next_batch` yields `batch_size` pages
/// at a time (zero selects the default of 25); cross-page passes see only
/// their own batch, so output can differ from a whole-document parse. Fails
/// with `LITEPARSE_STATUS_INVALID_CONFIG` when `target_pages` is configured.
/// Sessions are single-owner: `next_batch` mutates the handle.
///
/// # Safety
///
/// Same contract as `liteparse_parse_path`, with `out_session` in place of
/// `out_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_session_open_path(
    parser: *const LiteParseParser,
    path: *const c_char,
    batch_size: usize,
    out_session: *mut *mut LiteParseSession,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_session, "out_session")?;
        let path = read_utf8(path, "path")?;
        let state = open_session(parser, PdfInput::Path(path), batch_size)?;
        emit_handle(out_session, || Ok(state))
    })
}

/// Open an in-memory document for bounded-memory batch parsing. The input
/// bytes are copied before opening; `data` follows `liteparse_parse_bytes`.
/// See `liteparse_session_open_path` for session semantics.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_session_open_bytes(
    parser: *const LiteParseParser,
    data: *const u8,
    data_len: usize,
    batch_size: usize,
    out_session: *mut *mut LiteParseSession,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_session, "out_session")?;
        let bytes = copy_input_bytes(data, data_len)?;
        let state = open_session(parser, PdfInput::Bytes(bytes), batch_size)?;
        emit_handle(out_session, || Ok(state))
    })
}

/// Destroy a session handle, releasing the converted temporary PDF for a
/// non-PDF source. A null pointer is accepted as a no-op. Must not race with
/// an active `liteparse_session_next_batch` call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_session_free(session: *mut LiteParseSession) {
    // SAFETY: The pointer is either null or a handle returned by this crate.
    unsafe { free_handle(session) };
}

/// Return the total pages in the source document (before `max_pages` or
/// batching), or zero for a null/invalid handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_session_get_total_pages(
    session: *const LiteParseSession,
) -> u32 {
    unsafe {
        state_ref(session)
            .ok()
            .map_or(0, |state| state.session.total_pages())
    }
}

/// Parse and return the next batch as a typed result handle, with the
/// batch's inclusive 1-based page range in the non-null page outputs. Once
/// every page within `max_pages` has been yielded, returns
/// `LITEPARSE_STATUS_OK` with `*out_result` left null.
///
/// # Safety
///
/// `session` must be a live handle with no concurrent call using it; each
/// output slot must be valid for one non-overlapping write (page and error
/// outputs may be null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_session_next_batch(
    session: *mut LiteParseSession,
    out_result: *mut *mut LiteParseResult,
    out_start_page: *mut u32,
    out_end_page: *mut u32,
    out_error: *mut *mut c_char,
) -> LiteParseStatus {
    unsafe {
        // SAFETY: Non-null output pointers are writable by the caller contract.
        write_out(out_start_page, None);
        write_out(out_end_page, None);
    }
    ffi_boundary(out_error, || unsafe {
        prepare_out(out_result, "out_result")?;
        let state = state_mut(session)?;
        let Some(batch) = shared_runtime()?
            .block_on(state.session.next_batch())
            .map_err(map_parse_error)?
        else {
            return Ok(());
        };
        write_out(out_start_page, Some(batch.start_page));
        write_out(out_end_page, Some(batch.end_page));
        let extract_text_metadata = state.extract_text_metadata;
        emit_handle(out_result, || {
            Ok(ResultState {
                result: batch.result,
                extract_text_metadata,
            })
        })
    })
}

/// Free an owned string returned by this library (never the static
/// `liteparse_version` pointer). A null pointer is accepted as a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_string_free(string: *mut c_char) {
    if string.is_null() {
        return;
    }
    suppress_panics(|| {
        // SAFETY: Required by this function's caller contract.
        unsafe { drop(CString::from_raw(string)) };
    });
}

/// Validate an output slot and clear any stale value, so failed calls leave
/// null behind. `emit_handle`/`write_string` rely on the slot being prepared.
unsafe fn prepare_out<T>(out: *mut *mut T, name: &str) -> FfiResult {
    if out.is_null() {
        return Err(FfiError::invalid_argument(format!(
            "{name} must not be null"
        )));
    }
    // SAFETY: The caller contract requires writable output storage.
    unsafe { out.write(ptr::null_mut()) };
    Ok(())
}

/// Run one blocking parse on the parser's runtime, returning the result and
/// the `extract_text_metadata` flag its JSON serialization needs.
unsafe fn run_parse(
    parser: *const LiteParseParser,
    input: PdfInput,
) -> FfiResult<(ParseResult, bool)> {
    // SAFETY: The caller contract requires a live parser handle.
    let state = unsafe { state_ref(parser) }?;
    let result = shared_runtime()?
        .block_on(state.parser.parse_input(input))
        .map_err(map_parse_error)?;
    Ok((result, state.parser.config().extract_text_metadata))
}

/// Run one blocking screenshot render on the shared runtime.
unsafe fn run_screenshot(
    parser: *const LiteParseParser,
    input: PdfInput,
    page_numbers: Option<Vec<u32>>,
) -> FfiResult<Vec<ScreenshotResult>> {
    // SAFETY: The caller contract requires a live parser handle.
    let state = unsafe { state_ref(parser) }?;
    shared_runtime()?
        .block_on(state.parser.screenshot_input(input, page_numbers))
        .map_err(map_parse_error)
}

/// Run one blocking complexity analysis on the shared runtime.
unsafe fn run_is_complex(
    parser: *const LiteParseParser,
    input: PdfInput,
) -> FfiResult<Vec<PageComplexityStats>> {
    // SAFETY: The caller contract requires a live parser handle.
    let state = unsafe { state_ref(parser) }?;
    shared_runtime()?
        .block_on(state.parser.is_complex(input))
        .map_err(map_parse_error)
}

unsafe fn open_session(
    parser: *const LiteParseParser,
    input: PdfInput,
    batch_size: usize,
) -> FfiResult<SessionState> {
    // SAFETY: The caller contract requires a live parser handle.
    let state = unsafe { state_ref(parser) }?;
    let batch_size = if batch_size == 0 {
        DEFAULT_PAGE_BATCH_SIZE
    } else {
        batch_size
    };
    let session = shared_runtime()?
        .block_on(state.parser.open_batch_session(input, batch_size))
        .map_err(map_parse_error)?;
    Ok(SessionState {
        session,
        extract_text_metadata: state.parser.config().extract_text_metadata,
    })
}

unsafe fn copy_page_numbers(
    page_numbers: *const u32,
    page_numbers_len: usize,
) -> FfiResult<Option<Vec<u32>>> {
    if page_numbers.is_null() {
        if page_numbers_len != 0 {
            return Err(FfiError::invalid_argument(
                "page_numbers must not be null when page_numbers_len is non-zero",
            ));
        }
        return Ok(None);
    }
    if page_numbers_len > (isize::MAX as usize) / size_of::<u32>() {
        return Err(FfiError::invalid_argument(
            "page_numbers_len must not exceed isize::MAX bytes",
        ));
    }
    // SAFETY: The caller guarantees the allocation and the size bound.
    Ok(Some(
        unsafe { std::slice::from_raw_parts(page_numbers, page_numbers_len) }.to_vec(),
    ))
}

unsafe fn copy_input_bytes(data: *const u8, data_len: usize) -> FfiResult<Vec<u8>> {
    if data.is_null() && data_len != 0 {
        return Err(FfiError::invalid_argument(
            "data must not be null when data_len is non-zero",
        ));
    }
    if data_len > isize::MAX as usize {
        return Err(FfiError::invalid_argument(
            "data_len must not exceed isize::MAX",
        ));
    }
    if data_len == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: The caller guarantees the allocation and the size bound.
    Ok(unsafe { std::slice::from_raw_parts(data, data_len) }.to_vec())
}

/// The process-wide runtime every parser and session blocks on. Built on
/// first use, never shut down; this lets sessions outlive their parser.
fn shared_runtime() -> FfiResult<&'static tokio::runtime::Runtime> {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            FfiError::new(
                LITEPARSE_STATUS_RUNTIME_ERROR,
                format!("failed to create async runtime: {error}"),
            )
        })?;
    // A concurrent first use may have won the race; its runtime wins.
    Ok(RUNTIME.get_or_init(|| runtime))
}

fn build_parser_state(
    config: LiteParseConfig,
    ocr_callback: Option<CallbackOcrSpec>,
) -> FfiResult<ParserState> {
    let mut parser = liteparse::LiteParse::new(config);
    if let Some(spec) = ocr_callback {
        parser = parser.with_ocr_engine(std::sync::Arc::new(CallbackOcrEngine { spec }));
    }
    Ok(ParserState { parser })
}

/// Build a state value and write its boxed handle to `out`, which the
/// caller prepared with `prepare_out`.
unsafe fn emit_handle<H: Opaque>(
    out: *mut *mut H,
    build: impl FnOnce() -> FfiResult<H::State>,
) -> FfiResult {
    let handle = Box::into_raw(Box::new(build()?)).cast::<H>();
    // SAFETY: The caller validated `out` via `prepare_out`.
    unsafe { out.write(handle) };
    Ok(())
}

unsafe fn free_handle<H: Opaque>(handle: *mut H) {
    if handle.is_null() {
        return;
    }
    suppress_panics(|| {
        // SAFETY: The pointer is a live allocation produced by `emit_handle`.
        unsafe { drop(Box::from_raw(handle.cast::<H::State>())) };
    });
}

/// Run a destructor that must not unwind across the C ABI, leaking the panic
/// payload rather than dropping it (its own drop could panic again).
fn suppress_panics(operation: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(operation)) {
        std::mem::forget(payload);
    }
}

/// Write a looked-up view to a getter's out parameter, defaulting the slot
/// on a miss. Returns whether the lookup hit.
unsafe fn write_out<T: Default>(out: *mut T, value: Option<T>) -> bool {
    if out.is_null() {
        return false;
    }
    let found = value.is_some();
    // SAFETY: The caller contract requires writable output storage.
    unsafe { out.write(value.unwrap_or_default()) };
    found
}

/// Borrowed C view of one text item; valid while the owning handle lives.
fn text_item_view(item: &TextItem) -> LiteParseTextItem {
    LiteParseTextItem {
        text: byte_view(Some(item.text.as_str())),
        font_name: byte_view(item.font_name.as_deref()),
        link: byte_view(item.link.as_deref()),
        x: item.x,
        y: item.y,
        width: item.width,
        height: item.height,
        rotation: item.rotation,
        font_size: item.font_size.unwrap_or_default(),
        has_font_size: item.font_size.is_some(),
        confidence: item.confidence.unwrap_or_default(),
        has_confidence: item.confidence.is_some(),
    }
}

fn byte_view(value: Option<&str>) -> LiteParseByteView {
    value.map_or_else(LiteParseByteView::default, |value| {
        bytes_view(value.as_bytes())
    })
}

fn bytes_view(value: &[u8]) -> LiteParseByteView {
    LiteParseByteView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

fn map_parse_error(error: liteparse::LiteParseError) -> FfiError {
    let status = if matches!(&error, liteparse::LiteParseError::Config(_)) {
        LITEPARSE_STATUS_INVALID_CONFIG
    } else {
        LITEPARSE_STATUS_PARSE_ERROR
    };
    FfiError::new(status, error.to_string())
}

fn format_result(result: &ParseResult, extract_text_metadata: bool) -> FfiResult<String> {
    let mut value = liteparse::output::json::json_result_value(result, extract_text_metadata)
        .map_err(FfiError::serialization)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| FfiError::serialization("core parse JSON was not an object"))?;
    object.insert("text".into(), Value::String(result.text.clone()));
    insert_optional_string(object, "creator", result.creator.clone());
    insert_optional_string(object, "producer", result.producer.clone());
    if let Some(metadata) = result.doc_meta.clone() {
        let metadata = serde_json::to_value(metadata).map_err(FfiError::serialization)?;
        object.insert("doc_meta".into(), metadata);
    }
    serde_json::to_string_pretty(&value).map_err(FfiError::serialization)
}

fn insert_optional_string(object: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value {
        object.insert(key.into(), Value::String(value));
    }
}

unsafe fn read_utf8(pointer: *const c_char, name: &str) -> FfiResult<String> {
    if pointer.is_null() {
        return Err(FfiError::invalid_argument(format!(
            "{name} must not be null"
        )));
    }
    // SAFETY: The caller contract guarantees a readable NUL-terminated string.
    let value = unsafe { CStr::from_ptr(pointer) };
    value
        .to_str()
        .map(str::to_owned)
        .map_err(|error| FfiError::invalid_argument(format!("{name} is not valid UTF-8: {error}")))
}

unsafe fn write_string(out: *mut *mut c_char, value: String) -> FfiResult {
    let value = CString::new(value)
        .map_err(|_| FfiError::serialization("serialized output contained an interior NUL byte"))?;
    // SAFETY: The caller contract guarantees that `out` is writable.
    unsafe { out.write(value.into_raw()) };
    Ok(())
}

/// `ffi_boundary` for status-only exports with no error-string channel (the
/// OCR sink functions).
fn catch_status(operation: impl FnOnce() -> FfiResult) -> LiteParseStatus {
    ffi_boundary(ptr::null_mut(), operation)
}

fn ffi_boundary<F>(out_error: *mut *mut c_char, operation: F) -> LiteParseStatus
where
    F: FnOnce() -> FfiResult,
{
    if !out_error.is_null() {
        // SAFETY: Every exported caller documents this contract.
        unsafe { out_error.write(ptr::null_mut()) };
    }
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => LITEPARSE_STATUS_OK,
        Ok(Err(error)) => {
            // SAFETY: Every exported caller documents this contract.
            unsafe { write_error(out_error, &error.message) };
            error.status
        }
        Err(payload) => {
            let message = panic_message(payload.as_ref());
            std::mem::forget(payload);
            // SAFETY: Every exported caller documents this contract.
            unsafe { write_error(out_error, &format!("Rust panic: {message}")) };
            LITEPARSE_STATUS_PANIC
        }
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

unsafe fn write_error(out_error: *mut *mut c_char, message: &str) {
    if out_error.is_null() {
        return;
    }
    // Sanitize only when an interior NUL actually appears.
    let error = CString::new(message).unwrap_or_else(|_| {
        CString::new(message.replace('\0', "\\0"))
            .expect("sanitized error strings contain no NUL bytes")
    });
    // SAFETY: The caller contract guarantees that `out_error` is writable.
    unsafe { out_error.write(error.into_raw()) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../integration_tests_data")
            .join(name)
            .to_string_lossy()
            .into_owned()
    }

    fn sample_pdf_path() -> String {
        fixture_path("sample.pdf")
    }

    /// Build a parser from quiet, OCR-off base options; `configure` applies
    /// any test-specific setters on top before the parser is created.
    fn new_parser_with(
        configure: impl FnOnce(*mut LiteParseOptions, &mut *mut c_char),
    ) -> *mut LiteParseParser {
        let mut options = ptr::null_mut();
        let mut parser = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and writable for these calls.
        assert_eq!(
            unsafe { liteparse_options_new(&mut options, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert_eq!(
            unsafe { liteparse_options_set_ocr_enabled(options, false, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert_eq!(
            unsafe { liteparse_options_set_quiet(options, true, &mut error) },
            LITEPARSE_STATUS_OK
        );
        configure(options, &mut error);
        assert_eq!(
            unsafe { liteparse_parser_new_with_options(options, &mut parser, &mut error) },
            LITEPARSE_STATUS_OK
        );
        // SAFETY: `options` is a live library-owned handle no longer needed by the parser.
        unsafe { liteparse_options_free(options) };
        assert!(error.is_null());
        parser
    }

    fn new_test_parser() -> *mut LiteParseParser {
        new_parser_with(|_, _| {})
    }

    #[test]
    fn parser_builder_rejects_null_options_and_owns_its_error() {
        let mut parser = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: Output pointers are live; null options is deliberately invalid.
        let status =
            unsafe { liteparse_parser_new_with_options(ptr::null(), &mut parser, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(parser.is_null());
        assert!(!error.is_null());
        // SAFETY: `error` is a live library-owned string.
        unsafe { liteparse_string_free(error) };
    }

    #[test]
    fn c_constructor_and_destructor_accept_default_like_config() {
        let parser = new_test_parser();
        assert!(!parser.is_null());
        // SAFETY: `parser` is a live library-owned handle.
        unsafe { liteparse_parser_free(parser) };
    }

    #[test]
    fn typed_options_builder_validates_and_builds_parser() {
        let mut options = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and writable for this call.
        assert_eq!(
            unsafe { liteparse_options_new(&mut options, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert!(!options.is_null());
        assert!(error.is_null());

        // SAFETY: `options` is a live handle and the output error slot is valid.
        assert_eq!(
            unsafe { liteparse_options_set_ocr_enabled(options, false, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert_eq!(
            unsafe { liteparse_options_set_num_workers(options, 0, &mut error) },
            LITEPARSE_STATUS_INVALID_CONFIG
        );
        assert!(!error.is_null());
        // SAFETY: `error` is a live library-owned string.
        unsafe { liteparse_string_free(error) };
        error = ptr::null_mut();
        assert_eq!(
            unsafe {
                liteparse_options_set_output_format(
                    options,
                    LITEPARSE_OUTPUT_FORMAT_MARKDOWN,
                    &mut error,
                )
            },
            LITEPARSE_STATUS_OK
        );

        let mut parser = ptr::null_mut();
        // SAFETY: The options and output slots are live for this call.
        assert_eq!(
            unsafe { liteparse_parser_new_with_options(options, &mut parser, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert!(!parser.is_null());
        // SAFETY: Handles are live and owned by this test.
        unsafe {
            liteparse_parser_free(parser);
            liteparse_options_free(options);
        }
    }

    #[test]
    fn allocating_entry_points_clear_stale_outputs_before_validation() {
        let mut parser = ptr::dangling_mut::<LiteParseParser>();
        let mut error = ptr::null_mut();
        // SAFETY: The output pointer is valid; null options is deliberately invalid.
        let status =
            unsafe { liteparse_parser_new_with_options(ptr::null(), &mut parser, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(parser.is_null());
        // SAFETY: `error` is a live library-owned string.
        unsafe { liteparse_string_free(error) };

        let mut result = ptr::dangling_mut::<LiteParseResult>();
        // SAFETY: Null input is deliberately invalid; output storage is live.
        let status = unsafe {
            liteparse_parse_path_result(ptr::null(), ptr::null(), &mut result, ptr::null_mut())
        };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(result.is_null());
    }

    #[test]
    fn typed_result_getters_are_borrowed_and_json_round_trips() {
        let parser = new_test_parser();
        let sample = sample_pdf_path();
        let path = CString::new(sample).unwrap();
        let mut result = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(error.is_null());
        assert!(!result.is_null());
        assert!(unsafe { liteparse_result_get_total_pages(result) } > 0);
        assert!(unsafe { liteparse_result_get_page_count(result) } > 0);

        let mut text = LiteParseByteView {
            ptr: ptr::null(),
            len: usize::MAX,
        };
        // SAFETY: `result` is live and `text` is writable.
        assert!(unsafe { liteparse_result_get_text(result, &mut text) });
        assert!(!text.ptr.is_null());
        let page_count = unsafe { liteparse_result_get_page_count(result) };
        assert!(unsafe { liteparse_result_get_page_number(result, 0) } >= 1);
        assert_eq!(
            unsafe { liteparse_result_get_page_number(result, page_count) },
            0
        );

        let mut item = LiteParseTextItem::default();
        let item_count = unsafe { liteparse_result_get_text_item_count(result, 0) };
        if item_count > 0 {
            // SAFETY: `result` is live and `item` is writable.
            assert!(unsafe { liteparse_result_get_text_item(result, 0, 0, &mut item) });
            assert!(!item.text.ptr.is_null());
        }

        let mut json = ptr::null_mut();
        // SAFETY: `result` is live and output slots are writable.
        assert_eq!(
            unsafe { liteparse_result_to_json(result, &mut json, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert!(!json.is_null());
        // SAFETY: Values are live library-owned strings/handles.
        unsafe {
            liteparse_string_free(json);
            liteparse_string_free(error);
            liteparse_result_free(result);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn c_parse_bytes_rejects_null_non_empty_input() {
        let parser = new_test_parser();
        let mut json = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: Deliberately invalid input exercises argument validation.
        let status =
            unsafe { liteparse_parse_bytes(parser, ptr::null(), 1, &mut json, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(json.is_null());
        assert!(!error.is_null());
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(error);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn c_parse_bytes_rejects_lengths_larger_than_isize_max() {
        let parser = new_test_parser();
        let mut json = ptr::null_mut();
        let mut error = ptr::null_mut();
        let data = ptr::NonNull::<u8>::dangling().as_ptr();
        // SAFETY: Oversized length is rejected before the pointer is read.
        let status = unsafe {
            liteparse_parse_bytes(parser, data, isize::MAX as usize + 1, &mut json, &mut error)
        };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(error);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn screenshots_render_pages_and_reject_bad_page_number_input() {
        let parser = new_test_parser();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut screenshots = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live; null selection renders every page.
        let status = unsafe {
            liteparse_screenshot_path(
                parser,
                path.as_ptr(),
                ptr::null(),
                0,
                &mut screenshots,
                &mut error,
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(error.is_null());
        let count = unsafe { liteparse_screenshots_get_count(screenshots) };
        assert!(count > 0);

        let mut screenshot = LiteParseScreenshot::default();
        // SAFETY: `screenshots` is live and `screenshot` is writable.
        assert!(unsafe { liteparse_screenshots_get(screenshots, 0, &mut screenshot) });
        assert!(screenshot.page_number >= 1);
        assert!(screenshot.width > 0 && screenshot.height > 0);
        assert!(!screenshot.png.ptr.is_null() && screenshot.png.len > 0);
        assert!(!unsafe { liteparse_screenshots_get(screenshots, count, &mut screenshot) });
        // SAFETY: `screenshots` is a live library-owned handle.
        unsafe { liteparse_screenshots_free(screenshots) };

        // A single-page selection renders exactly one page.
        let pages = [1u32];
        let mut selected = ptr::null_mut();
        // SAFETY: The selection array is live for the duration of the call.
        let status = unsafe {
            liteparse_screenshot_path(
                parser,
                path.as_ptr(),
                pages.as_ptr(),
                pages.len(),
                &mut selected,
                &mut error,
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert_eq!(unsafe { liteparse_screenshots_get_count(selected) }, 1);
        // SAFETY: `selected` is a live library-owned handle.
        unsafe { liteparse_screenshots_free(selected) };

        let mut invalid = ptr::null_mut();
        // SAFETY: A null selection with non-zero length is deliberately invalid.
        let status = unsafe {
            liteparse_screenshot_path(
                parser,
                path.as_ptr(),
                ptr::null(),
                1,
                &mut invalid,
                &mut error,
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(invalid.is_null());
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(error);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn complexity_reports_pages_and_serializes_full_json() {
        let parser = new_test_parser();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut complexity = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live for this call.
        let status = unsafe {
            liteparse_is_complex_path(parser, path.as_ptr(), &mut complexity, &mut error)
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(error.is_null());
        let count = unsafe { liteparse_complexity_get_count(complexity) };
        assert!(count > 0);

        let mut page = LiteParsePageComplexity::default();
        // SAFETY: `complexity` is live and `page` is writable.
        assert!(unsafe { liteparse_complexity_get(complexity, 0, &mut page) });
        assert!(page.page_number >= 1);
        assert!(page.page_area > 0.0);
        assert!(!unsafe { liteparse_complexity_get(complexity, count, &mut page) });

        let mut json = ptr::null_mut();
        // SAFETY: `complexity` is live and output slots are writable.
        assert_eq!(
            unsafe { liteparse_complexity_to_json(complexity, &mut json, &mut error) },
            LITEPARSE_STATUS_OK
        );
        assert!(!json.is_null());
        // SAFETY: `json` is a live NUL-terminated library-owned string.
        let text = unsafe { CStr::from_ptr(json) }.to_str().unwrap();
        assert!(text.contains("needs_ocr"));
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(json);
            liteparse_complexity_free(complexity);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn batch_session_yields_every_page_and_outlives_its_parser() {
        let parser = new_test_parser();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut session = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live for this call.
        let status = unsafe {
            liteparse_session_open_path(parser, path.as_ptr(), 1, &mut session, &mut error)
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(error.is_null());
        // The session must stay usable after its parser is freed.
        // SAFETY: `parser` is a live handle with no active call.
        unsafe { liteparse_parser_free(parser) };

        let total_pages = unsafe { liteparse_session_get_total_pages(session) };
        assert!(total_pages > 0);

        let mut pages_seen = 0u32;
        let mut last_end = 0u32;
        loop {
            let mut result = ptr::null_mut();
            let mut start_page = u32::MAX;
            let mut end_page = u32::MAX;
            // SAFETY: `session` is live and all output slots are writable.
            let status = unsafe {
                liteparse_session_next_batch(
                    session,
                    &mut result,
                    &mut start_page,
                    &mut end_page,
                    &mut error,
                )
            };
            assert_eq!(status, LITEPARSE_STATUS_OK);
            if result.is_null() {
                assert_eq!(start_page, 0);
                assert_eq!(end_page, 0);
                break;
            }
            assert_eq!(start_page, last_end + 1);
            assert!(end_page >= start_page);
            last_end = end_page;
            pages_seen += end_page - start_page + 1;
            assert_eq!(
                unsafe { liteparse_result_get_page_count(result) },
                (end_page - start_page + 1) as usize
            );
            // SAFETY: `result` is a live library-owned handle.
            unsafe { liteparse_result_free(result) };
        }
        assert_eq!(pages_seen, total_pages);
        // SAFETY: `session` is a live library-owned handle.
        unsafe { liteparse_session_free(session) };
    }

    #[test]
    fn batch_session_rejects_target_pages() {
        let pages = CString::new("1").unwrap();
        let parser = new_parser_with(|options, error| {
            // SAFETY: The helper supplies a live options handle and error slot.
            assert_eq!(
                unsafe { liteparse_options_set_target_pages(options, pages.as_ptr(), error) },
                LITEPARSE_STATUS_OK
            );
        });

        let mut error = ptr::null_mut();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut session = ptr::null_mut();
        // SAFETY: All pointers are live; the config conflict is the point.
        let status = unsafe {
            liteparse_session_open_path(parser, path.as_ptr(), 0, &mut session, &mut error)
        };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_CONFIG);
        assert!(session.is_null());
        assert!(!error.is_null());
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(error);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn word_boxes_require_opt_in_and_expose_word_geometry() {
        let parser = new_parser_with(|options, error| {
            // SAFETY: The helper supplies a live options handle and error slot.
            assert_eq!(
                unsafe { liteparse_options_set_emit_word_boxes(options, true, error) },
                LITEPARSE_STATUS_OK
            );
        });

        let mut error = ptr::null_mut();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut result = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_OK);

        let item_count = unsafe { liteparse_result_get_text_item_count(result, 0) };
        let with_words = (0..item_count)
            .find(|&item| unsafe { liteparse_result_get_word_box_count(result, 0, item) } > 0);
        let item = with_words.expect("opted-in parse should split some item into words");
        let mut word = LiteParseWordBox::default();
        // SAFETY: `result` is live and `word` is writable.
        assert!(unsafe { liteparse_result_get_word_box(result, 0, item, 0, &mut word) });
        assert!(!word.text.ptr.is_null() && word.text.len > 0);
        assert!(word.width > 0.0 && word.height > 0.0);
        let words = unsafe { liteparse_result_get_word_box_count(result, 0, item) };
        assert!(!unsafe { liteparse_result_get_word_box(result, 0, item, words, &mut word) });
        // SAFETY: Handles are live and owned by this test.
        unsafe {
            liteparse_result_free(result);
            liteparse_parser_free(parser);
        }

        // Without the opt-in, every item reports zero word boxes.
        let parser = new_test_parser();
        let mut result = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        let item_count = unsafe { liteparse_result_get_text_item_count(result, 0) };
        assert!(
            (0..item_count)
                .all(|item| unsafe { liteparse_result_get_word_box_count(result, 0, item) } == 0)
        );
        // SAFETY: Handles are live and owned by this test.
        unsafe {
            liteparse_result_free(result);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn search_finds_phrases_and_matches_outlive_the_result() {
        let parser = new_test_parser();
        let path = CString::new(sample_pdf_path()).unwrap();
        let mut result = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_OK);

        // Search for the first word of the first page, so the test tracks the
        // fixture instead of hard-coding its content.
        let mut page_text = LiteParseByteView::default();
        // SAFETY: `result` is live and `page_text` is writable.
        assert!(unsafe { liteparse_result_get_page_text(result, 0, &mut page_text) });
        // SAFETY: The view borrows UTF-8 text from the live result handle.
        let text = std::str::from_utf8(unsafe {
            std::slice::from_raw_parts(page_text.ptr, page_text.len)
        })
        .unwrap();
        let word = text.split_whitespace().next().expect("page has text");
        let phrase = CString::new(word).unwrap();

        let mut matches = ptr::null_mut();
        // SAFETY: All pointers are live for this call.
        let status = unsafe {
            liteparse_result_search(result, 0, phrase.as_ptr(), false, &mut matches, &mut error)
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(unsafe { liteparse_search_matches_get_count(matches) } > 0);

        // An empty phrase matches nothing; an out-of-range page is an error.
        let empty = CString::new("").unwrap();
        let mut empty_matches = ptr::null_mut();
        // SAFETY: All pointers are live for this call.
        let status = unsafe {
            liteparse_result_search(
                result,
                0,
                empty.as_ptr(),
                false,
                &mut empty_matches,
                &mut error,
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert_eq!(
            unsafe { liteparse_search_matches_get_count(empty_matches) },
            0
        );
        // SAFETY: `empty_matches` is a live library-owned handle.
        unsafe { liteparse_search_matches_free(empty_matches) };

        let page_count = unsafe { liteparse_result_get_page_count(result) };
        let mut bad_matches = ptr::null_mut();
        // SAFETY: The out-of-range page index is deliberately invalid.
        let status = unsafe {
            liteparse_result_search(
                result,
                page_count,
                phrase.as_ptr(),
                false,
                &mut bad_matches,
                &mut error,
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_INVALID_ARGUMENT);
        assert!(bad_matches.is_null());
        // SAFETY: `error` is a live library-owned string.
        unsafe { liteparse_string_free(error) };

        // Matches are copies: they stay readable after the result is freed.
        // SAFETY: `result` has no other user at this point.
        unsafe { liteparse_result_free(result) };
        let mut item = LiteParseTextItem::default();
        // SAFETY: `matches` is live and `item` is writable.
        assert!(unsafe { liteparse_search_matches_get(matches, 0, &mut item) });
        assert!(!item.text.ptr.is_null() && item.text.len >= word.len());
        assert!(item.width > 0.0 && item.height > 0.0);
        // SAFETY: Handles are live and owned by this test.
        unsafe {
            liteparse_search_matches_free(matches);
            liteparse_parser_free(parser);
        }
    }

    fn receipt_png_path() -> String {
        fixture_path("receipt.png")
    }

    /// Test callback: verifies the raster contract, counts invocations via
    /// `user_data`, and emits one fixed word.
    unsafe extern "C" fn stub_ocr_recognize(
        user_data: *mut c_void,
        pixels: *const u8,
        pixels_len: usize,
        width: u32,
        height: u32,
        pixel_format: u32,
        language: *const c_char,
        _dpi: f32,
        sink: *mut LiteParseOcrSink,
    ) -> u32 {
        assert!(!pixels.is_null());
        assert_eq!(pixel_format, LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE);
        assert_eq!(pixels_len, width as usize * height as usize);
        // SAFETY: The engine passes the configured NUL-terminated language.
        assert_eq!(unsafe { CStr::from_ptr(language) }.to_str().unwrap(), "eng");
        // SAFETY: `user_data` is the test's live invocation counter.
        unsafe { &*user_data.cast::<std::sync::atomic::AtomicUsize>() }
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let text = CString::new("CALLBACKWORD").unwrap();
        // SAFETY: The sink is live for this invocation and text is valid.
        let status = unsafe {
            liteparse_ocr_sink_add(
                sink,
                text.as_ptr(),
                10.0,
                10.0,
                120.0,
                40.0,
                0.95,
                ptr::null(),
            )
        };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        0
    }

    /// Test callback: always fails after setting a message.
    unsafe extern "C" fn failing_ocr_recognize(
        _user_data: *mut c_void,
        _pixels: *const u8,
        _pixels_len: usize,
        _width: u32,
        _height: u32,
        _pixel_format: u32,
        _language: *const c_char,
        _dpi: f32,
        sink: *mut LiteParseOcrSink,
    ) -> u32 {
        let message = CString::new("engine exploded").unwrap();
        // SAFETY: The sink is live for this invocation and message is valid.
        unsafe { liteparse_ocr_sink_set_error(sink, message.as_ptr()) };
        7
    }

    fn new_ocr_parser(
        recognize: OcrRecognizeRaw,
        user_data: *mut c_void,
        prefers_grayscale: bool,
        ocr_failure_fatal: bool,
    ) -> *mut LiteParseParser {
        new_parser_with(|options, error| {
            // SAFETY: The helper supplies a live options handle and error slot.
            unsafe {
                assert_eq!(
                    liteparse_options_set_ocr_enabled(options, true, error),
                    LITEPARSE_STATUS_OK
                );
                assert_eq!(
                    liteparse_options_set_ocr_failure_fatal(options, ocr_failure_fatal, error),
                    LITEPARSE_STATUS_OK
                );
                assert_eq!(
                    liteparse_options_set_ocr_callback(
                        options,
                        Some(recognize),
                        user_data,
                        ptr::null(),
                        prefers_grayscale,
                        error,
                    ),
                    LITEPARSE_STATUS_OK
                );
            }
        })
    }

    #[test]
    fn ocr_callback_engine_supplies_text_for_image_documents() {
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let parser = new_ocr_parser(
            stub_ocr_recognize,
            ptr::from_ref(&calls).cast_mut().cast::<c_void>(),
            true,
            true,
        );
        let path = CString::new(receipt_png_path()).unwrap();
        let mut result = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_OK);
        assert!(error.is_null());
        assert!(calls.load(std::sync::atomic::Ordering::SeqCst) > 0);

        let mut text = LiteParseByteView::default();
        // SAFETY: `result` is live and `text` is writable.
        assert!(unsafe { liteparse_result_get_text(result, &mut text) });
        // SAFETY: The view borrows UTF-8 text from the live result handle.
        let text =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(text.ptr, text.len) }).unwrap();
        assert!(text.contains("CALLBACKWORD"), "text was {text:?}");
        // SAFETY: Handles are live and owned by this test.
        unsafe {
            liteparse_result_free(result);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn ocr_callback_failure_message_reaches_the_caller_when_fatal() {
        let parser = new_ocr_parser(failing_ocr_recognize, ptr::null_mut(), false, true);
        let path = CString::new(receipt_png_path()).unwrap();
        let mut result = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and the parser remains alive.
        let status =
            unsafe { liteparse_parse_path_result(parser, path.as_ptr(), &mut result, &mut error) };
        assert_eq!(status, LITEPARSE_STATUS_PARSE_ERROR);
        assert!(result.is_null());
        assert!(!error.is_null());
        // SAFETY: `error` is a live NUL-terminated library-owned string.
        let message = unsafe { CStr::from_ptr(error) }.to_str().unwrap();
        assert!(message.contains("engine exploded"), "error was {message:?}");
        // SAFETY: Values are live and library-owned.
        unsafe {
            liteparse_string_free(error);
            liteparse_parser_free(parser);
        }
    }

    #[test]
    fn ocr_callback_registration_can_be_cleared() {
        let mut options = ptr::null_mut();
        let mut error = ptr::null_mut();
        // SAFETY: All pointers are live and writable for these calls.
        unsafe {
            assert_eq!(
                liteparse_options_new(&mut options, &mut error),
                LITEPARSE_STATUS_OK
            );
            assert_eq!(
                liteparse_options_set_ocr_callback(
                    options,
                    Some(stub_ocr_recognize),
                    ptr::null_mut(),
                    ptr::null(),
                    false,
                    &mut error,
                ),
                LITEPARSE_STATUS_OK
            );
            assert_eq!(
                liteparse_options_set_ocr_callback(
                    options,
                    None,
                    ptr::null_mut(),
                    ptr::null(),
                    false,
                    &mut error,
                ),
                LITEPARSE_STATUS_OK
            );
        }
        // SAFETY: `options` is a live library-owned handle.
        let state = unsafe { &*options.cast::<OptionsState>() };
        assert!(state.ocr_callback.is_none());
        // SAFETY: `options` is a live library-owned handle.
        unsafe { liteparse_options_free(options) };
    }

    #[test]
    fn ffi_boundary_contains_panics() {
        let mut error = ptr::null_mut();
        let status = ffi_boundary(&mut error, || panic!("ffi panic test"));
        assert_eq!(status, LITEPARSE_STATUS_PANIC);
        assert!(!error.is_null());
        // SAFETY: `error` is a live library-owned string.
        unsafe { liteparse_string_free(error) };
    }

    #[test]
    fn parser_handle_supports_concurrent_parse_calls() {
        let parser = new_test_parser();
        let parser_address = parser as usize;
        let sample = sample_pdf_path();
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let sample = sample.clone();
                std::thread::spawn(move || {
                    let parser = parser_address as *const LiteParseParser;
                    let path = CString::new(sample).unwrap();
                    let mut json = ptr::null_mut();
                    let mut error = ptr::null_mut();
                    // SAFETY: Parser stays live until all workers join.
                    let status = unsafe {
                        liteparse_parse_path(parser, path.as_ptr(), &mut json, &mut error)
                    };
                    if !error.is_null() {
                        // SAFETY: `error` is a live library-owned string.
                        unsafe { liteparse_string_free(error) };
                    }
                    assert_eq!(status, LITEPARSE_STATUS_OK);
                    assert!(!json.is_null());
                    // SAFETY: `json` is a live library-owned string.
                    unsafe { liteparse_string_free(json) };
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        // SAFETY: All parse calls have joined.
        unsafe { liteparse_parser_free(parser) };
    }
}
