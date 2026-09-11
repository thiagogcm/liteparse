use std::ffi::{c_char, c_void};
use std::sync::{Arc, RwLock};

use liteparse::ocr::OcrEngine;
use liteparse::{
    FontDbResolver, GlyphResolver, LiteParse as CoreLiteParse, LiteParseConfig as CoreConfig,
};

use crate::config::{LiteParseConfig, owned_config};
use crate::handle::{
    LiteParseByteView, build_handle, free_handle, opaque_handles, required_view_str, state_ref,
};
use crate::ocr::{CallbackOcrEngine, LiteParseOcrRecognizeFn};
use crate::status::{LiteParseStatus, boundary};

/// An owned parser. Safe to share between threads; destruction must wait for
/// in-flight operations.
pub struct LiteParseParser {
    _opaque: [u8; 0],
}

/// Status and handle returned by `liteparse_parser_new`. The handle is null
/// unless the status is `LITEPARSE_STATUS_OK`.
#[repr(C)]
pub struct LiteParseParserNew {
    pub status: LiteParseStatus,
    pub handle: *mut LiteParseParser,
}

opaque_handles! {
    LiteParseParser => ParserState, "parser";
}

pub(crate) struct ParserState {
    config: CoreConfig,
    glyph_resolver: Option<Arc<dyn GlyphResolver>>,
    /// Documents snapshot the engine when opened.
    ocr_engine: RwLock<Option<Arc<dyn OcrEngine>>>,
}

impl ParserState {
    pub(crate) fn config(&self) -> &CoreConfig {
        &self.config
    }

    pub(crate) fn glyph_resolver(&self) -> Option<Arc<dyn GlyphResolver>> {
        self.glyph_resolver.clone()
    }

    pub(crate) fn ocr_engine(&self) -> Option<Arc<dyn OcrEngine>> {
        self.ocr_engine
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

pub(crate) fn build_parser(
    config: CoreConfig,
    ocr_engine: Option<Arc<dyn OcrEngine>>,
    glyph_resolver: Option<Arc<dyn GlyphResolver>>,
) -> CoreLiteParse {
    let parser = CoreLiteParse::new(config);
    let parser = match glyph_resolver {
        Some(resolver) => parser.with_glyph_resolver(resolver),
        None => parser,
    };
    match ocr_engine {
        Some(engine) => parser.with_ocr_engine(engine),
        None => parser,
    }
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ParserState>();
};

/// Return the static, NUL-terminated binding version.
#[unsafe(no_mangle)]
pub extern "C" fn liteparse_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Create a parser and copy its configuration.
///
/// # Safety
///
/// `config` and its views must be readable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_new(
    config: *const LiteParseConfig,
) -> LiteParseParserNew {
    let (status, handle) = build_handle(|| {
        let owned = unsafe { owned_config(config) }?;
        Ok(ParserState {
            config: owned.core,
            glyph_resolver: owned
                .font_db_dir
                .map(|dir| Arc::new(FontDbResolver::new(dir)) as Arc<dyn GlyphResolver>),
            ocr_engine: RwLock::new(None),
        })
    });
    LiteParseParserNew { status, handle }
}

/// Register or clear an OCR callback. Open documents retain their callback.
///
/// # Safety
///
/// The callback and `user_data` must remain valid and thread-safe while the
/// parser or any document opened from it lives. `name` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_set_ocr_callback(
    parser: *const LiteParseParser,
    recognize: LiteParseOcrRecognizeFn,
    user_data: *mut c_void,
    name: LiteParseByteView,
    prefers_grayscale: bool,
) -> LiteParseStatus {
    boundary(|| {
        let state = unsafe { state_ref(parser) }?;
        let engine: Option<Arc<dyn OcrEngine>> = match recognize {
            None => None,
            Some(recognize) => {
                let name = if name.ptr.is_null() {
                    "c-callback".to_owned()
                } else {
                    unsafe { required_view_str(name, "name") }?
                };
                Some(Arc::new(CallbackOcrEngine::new(
                    recognize,
                    user_data,
                    name,
                    prefers_grayscale,
                )))
            }
        };
        *state
            .ocr_engine
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = engine;
        Ok(())
    })
}

/// Destroy a parser handle. Null is a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_free(parser: *mut LiteParseParser) {
    unsafe { free_handle(parser) };
}
