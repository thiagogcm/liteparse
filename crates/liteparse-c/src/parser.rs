use std::sync::Arc;

use liteparse::ocr::OcrEngine;
use liteparse::{
    FontDbResolver, GlyphResolver, LiteParse as CoreLiteParse, LiteParseConfig as CoreConfig,
};

use crate::config::{LiteParseConfig, owned_config};
use crate::handle::{create_handle, free_handle, opaque_handles};
use crate::status::{FfiError, LiteParseStatus};

/// An owned parser. Safe to share between threads; destruction must wait for
/// in-flight operations.
pub struct LiteParseParser {
    _opaque: [u8; 0],
}

opaque_handles! {
    LiteParseParser => ParserState, "parser";
}

pub(crate) struct ParserState {
    config: CoreConfig,
    glyph_resolver: Option<Arc<dyn GlyphResolver>>,
    /// The configured engine, or why OCR is enabled without one; built once
    /// and shared by every document.
    ocr_engine: OcrEngineChoice,
}

pub(crate) type OcrEngineChoice = Result<Option<Arc<dyn OcrEngine>>, FfiError>;

impl ParserState {
    pub(crate) fn config(&self) -> &CoreConfig {
        &self.config
    }

    pub(crate) fn glyph_resolver(&self) -> Option<Arc<dyn GlyphResolver>> {
        self.glyph_resolver.clone()
    }

    pub(crate) fn ocr_engine(&self) -> OcrEngineChoice {
        self.ocr_engine.clone()
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

/// Create a parser, copying its configuration.
///
/// `config` and its views must be readable for the call; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_new(
    config: *const LiteParseConfig,
    out: *mut *mut LiteParseParser,
) -> LiteParseStatus {
    unsafe {
        create_handle(out, || {
            let owned = owned_config(config)?;
            let glyph_resolver = owned
                .font_db_dir
                .map(|dir| Arc::new(FontDbResolver::new(dir)) as Arc<dyn GlyphResolver>);
            let ocr_engine = build_parser(owned.core.clone(), None, glyph_resolver.clone())
                .ocr_engine()
                .map_err(FfiError::from);
            Ok(ParserState {
                config: owned.core,
                glyph_resolver,
                ocr_engine,
            })
        })
    }
}

/// Destroy a parser handle. Null is a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_parser_free(parser: *mut LiteParseParser) {
    unsafe { free_handle(parser) };
}
