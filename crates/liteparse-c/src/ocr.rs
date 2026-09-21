use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::ptr;

use liteparse::ocr::{OcrEngine, OcrOptions, OcrResult};

use crate::handle::{
    LiteParseByteView, as_slice, bytes_view, opaque_handles, required_view_str, state_mut,
};
use crate::status::{FfiError, LiteParseStatus, boundary};

/// `liteparse_parser_set_ocr_callback` flags.
pub const LITEPARSE_OCR_FLAG_PREFERS_GRAYSCALE: u32 = 1 << 0;

/// `LiteParseOcrImage.pixel_format` values.
pub const LITEPARSE_OCR_PIXEL_FORMAT_RGB: u32 = 0;
pub const LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE: u32 = 1;

/// `LiteParseOcrWord.flags` bits.
pub const LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON: u32 = 1 << 0;

/// Valid only during the callback that receives it.
pub struct LiteParseOcrSink {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseOcrSink => OcrSinkState, "sink";
}

/// The raster handed to an OCR callback. Borrowed for the callback's
/// duration.
#[repr(C)]
pub struct LiteParseOcrImage {
    /// Tightly packed rows, 3 bytes per pixel for RGB or 1 for grayscale.
    pub pixels: LiteParseByteView,
    pub width: u32,
    pub height: u32,
    /// `LITEPARSE_OCR_PIXEL_FORMAT_*`.
    pub pixel_format: u32,
    pub dpi: f32,
    /// The configured OCR language.
    pub language: LiteParseByteView,
}

/// Return nonzero to fail recognition. Calls may be concurrent.
// Spelled out rather than `Option<OcrRecognizeRaw>`: cbindgen does not see
// through the alias. This signature and `OcrRecognizeRaw` must stay in step.
pub type LiteParseOcrRecognizeFn = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        image: *const LiteParseOcrImage,
        sink: *mut LiteParseOcrSink,
    ) -> u32,
>;

type OcrRecognizeRaw = unsafe extern "C" fn(
    user_data: *mut c_void,
    image: *const LiteParseOcrImage,
    sink: *mut LiteParseOcrSink,
) -> u32;

/// One recognized word. Box edges are raster pixels; `polygon` holds four
/// x/y corners in reading order when `HAS_POLYGON` is set.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct LiteParseOcrWord {
    pub text: LiteParseByteView,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub confidence: f32,
    pub polygon: [f32; 8],
    /// `LITEPARSE_OCR_WORD_FLAG_*` bits.
    pub flags: u32,
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
        let image = LiteParseOcrImage {
            pixels: bytes_view(image_data),
            width,
            height,
            pixel_format: if self.prefers_grayscale {
                LITEPARSE_OCR_PIXEL_FORMAT_GRAYSCALE
            } else {
                LITEPARSE_OCR_PIXEL_FORMAT_RGB
            },
            dpi: options.dpi,
            language: bytes_view(options.language.as_bytes()),
        };
        let mut sink = OcrSinkState {
            results: Vec::new(),
            error: None,
        };
        let status = unsafe {
            (self.recognize)(
                self.user_data,
                ptr::from_ref(&image),
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

/// Append recognized words atomically; invalid input appends nothing.
///
/// `sink` must belong to the current callback; `words` must be readable for
/// `count` entries, or null with zero count.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_ocr_sink_add(
    sink: *mut LiteParseOcrSink,
    words: *const LiteParseOcrWord,
    count: usize,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let incoming = as_slice(words, count, "words")?.unwrap_or_default();
        let parsed =
            incoming
                .iter()
                .enumerate()
                .map(|(index, word)| {
                    if word.flags & !LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON != 0 {
                        return Err(FfiError::invalid_argument(format!(
                            "words[{index}].flags contains unknown bits"
                        )));
                    }
                    let [x0, y0, x1, y1, x2, y2, x3, y3] = word.polygon;
                    Ok(OcrResult {
                        text: required_view_str(word.text, &format!("words[{index}].text"))?,
                        bbox: [word.x1, word.y1, word.x2, word.y2],
                        confidence: word.confidence,
                        polygon: (word.flags & LITEPARSE_OCR_WORD_FLAG_HAS_POLYGON != 0)
                            .then_some([[x0, y0], [x1, y1], [x2, y2], [x3, y3]]),
                    })
                })
                .collect::<Result<Vec<_>, FfiError>>()?;
        state_mut(sink)?.results.extend(parsed);
        Ok(())
    })
}

/// Set the callback's failure message.
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
