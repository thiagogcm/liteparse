use std::ffi::{CString, c_char, c_void};
use std::future::Future;
use std::pin::Pin;
use std::ptr;

use liteparse::ocr::{OcrEngine, OcrOptions, OcrResult};

use crate::handle::{LiteParseByteView, as_slice, opaque_handles, required_view_str, state_mut};
use crate::status::{FfiError, LiteParseStatus, boundary};

/// Pixel formats passed to a `LiteParseOcrRecognizeFn`.
pub const LITEPARSE_OCR_PIXEL_FORMAT_RGB: u32 = 0;
pub const LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE: u32 = 1;

/// Valid only during the callback that receives it.
pub struct LiteParseOcrSink {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseOcrSink => OcrSinkState, "sink";
}

/// Return nonzero to fail recognition. Calls may be concurrent.
// Kept spelled out rather than `Option<OcrRecognizeRaw>`: cbindgen does not
// see through the alias and would emit an opaque type instead of the function
// pointer. This signature and `OcrRecognizeRaw` must stay in step.
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

#[repr(C)]
pub struct LiteParseOcrWordIn {
    pub text_offset: usize,
    pub text_length: usize,
    /// Box edges in raster pixels: left, top, right, bottom.
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub confidence: f32,
    /// Four x/y corners in reading order when `has_polygon` is set.
    pub polygon: [f32; 8],
    pub has_polygon: bool,
}

pub(crate) struct OcrSinkState {
    results: Vec<OcrResult>,
    error: Option<String>,
}

pub(crate) struct CallbackOcrEngine {
    recognize: OcrRecognizeRaw,
    user_data: *mut c_void,
    name: String,
    prefers_grayscale: bool,
}

// SAFETY: the registration contract requires the callback and user data to
// be usable from any thread concurrently.
unsafe impl Send for CallbackOcrEngine {}
unsafe impl Sync for CallbackOcrEngine {}

impl CallbackOcrEngine {
    pub(crate) fn new(
        recognize: OcrRecognizeRaw,
        user_data: *mut c_void,
        name: String,
        prefers_grayscale: bool,
    ) -> Self {
        Self {
            recognize,
            user_data,
            name,
            prefers_grayscale,
        }
    }

    fn call(
        &self,
        image_data: &[u8],
        width: u32,
        height: u32,
        options: &OcrOptions,
    ) -> Result<Vec<OcrResult>, String> {
        let language = CString::new(options.language.as_str())
            .map_err(|_| "ocr_language contains an interior NUL byte".to_owned())?;
        let pixel_format = if self.prefers_grayscale {
            LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE
        } else {
            LITEPARSE_OCR_PIXEL_FORMAT_RGB
        };
        let mut sink = OcrSinkState {
            results: Vec::new(),
            error: None,
        };
        let status = unsafe {
            (self.recognize)(
                self.user_data,
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
    }
}

impl OcrEngine for CallbackOcrEngine {
    fn name(&self) -> &str {
        &self.name
    }

    fn prefers_grayscale(&self) -> bool {
        self.prefers_grayscale
    }

    fn recognize<'a, 'b: 'a, 'c: 'a>(
        &'a self,
        image_data: &'c [u8],
        width: u32,
        height: u32,
        options: &'b OcrOptions,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<OcrResult>, Box<dyn std::error::Error + Send + Sync>>>
                + Send
                + '_,
        >,
    > {
        let result = self.call(image_data, width, height, options);
        Box::pin(std::future::ready(result.map_err(Into::into)))
    }
}

fn polygon(corners: [f32; 8]) -> [[f32; 2]; 4] {
    let [x0, y0, x1, y1, x2, y2, x3, y3] = corners;
    [[x0, y0], [x1, y1], [x2, y2], [x3, y3]]
}

/// Append one OCR word. `polygon_corners` points to eight floats when present.
///
/// # Safety
///
/// `sink` must belong to the current callback and `text` must be readable UTF-8.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_add(
    sink: *mut LiteParseOcrSink,
    text: LiteParseByteView,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    confidence: f32,
    polygon_corners: *const f32,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let state = state_mut(sink)?;
        let text = required_view_str(text, "text")?;
        let polygon = polygon_corners
            .cast::<[f32; 8]>()
            .as_ref()
            .map(|corners| polygon(*corners));
        state.results.push(OcrResult {
            text,
            bbox: [x1, y1, x2, y2],
            confidence,
            polygon,
        });
        Ok(())
    })
}

/// Append OCR words atomically; invalid input appends nothing.
///
/// # Safety
///
/// `sink` must belong to the current callback; input arrays must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_add_batch(
    sink: *mut LiteParseOcrSink,
    blob: *const u8,
    blob_len: usize,
    words: *const LiteParseOcrWordIn,
    count: usize,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let text_blob = as_slice(blob, blob_len, "blob")?.unwrap_or_default();
        let incoming = as_slice(words, count, "words")?.unwrap_or_default();
        let parsed = incoming
            .iter()
            .map(|word| {
                let end = word
                    .text_offset
                    .checked_add(word.text_length)
                    .filter(|end| *end <= text_blob.len())
                    .ok_or_else(|| {
                        FfiError::invalid_argument("word text range falls outside the blob")
                    })?;
                let text =
                    std::str::from_utf8(&text_blob[word.text_offset..end]).map_err(|error| {
                        FfiError::invalid_argument(format!("word text is not valid UTF-8: {error}"))
                    })?;
                Ok(OcrResult {
                    text: text.to_owned(),
                    bbox: [word.x1, word.y1, word.x2, word.y2],
                    confidence: word.confidence,
                    polygon: word.has_polygon.then(|| polygon(word.polygon)),
                })
            })
            .collect::<Result<Vec<_>, FfiError>>()?;
        state_mut(sink)?.results.extend(parsed);
        Ok(())
    })
}

/// Set the callback's failure message.
///
/// # Safety
///
/// `sink` must belong to the current callback and `message` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_set_error(
    sink: *mut LiteParseOcrSink,
    message: LiteParseByteView,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let state = state_mut(sink)?;
        state.error = Some(required_view_str(message, "message")?);
        Ok(())
    })
}
