//! Struct-layout introspection for foreign consumers that describe exported
//! layouts by hand and verify them at load time.

use crate::budget::{
    LITEPARSE_MAX_CONTENT_BINARY_BYTES, LITEPARSE_MAX_CONTENT_INPUT_BYTES,
    LITEPARSE_MAX_RESULT_BYTES, LITEPARSE_MAX_SINGLE_BINARY_BYTES,
};
use crate::config::{LiteParseConfig, LiteParseHeader, LiteParsePageOrientationCorrection};
use crate::content::LiteParseContentInput;
use crate::document::LITEPARSE_MAX_SELECTED_PAGES;
use crate::document::{LiteParseDocumentInfo, LiteParseDocumentMetadata, LiteParseRenderRegion};
use crate::handle::write_out;
use crate::handle::{LiteParseArenas, LiteParseByteView, LiteParseStr};
use crate::job::LiteParseOcrRasterView;
use crate::ocr::{LiteParseOcrPageInput, LiteParseOcrRaster, LiteParseOcrWord};
use crate::page_objects::{LITEPARSE_MAX_OBJECT_NESTING_DEPTH, LITEPARSE_MAX_OBJECTS};
use crate::page_objects::{
    LiteParseClipPath, LiteParseMatrix, LiteParsePageObject, LiteParsePageObjectPage,
    LiteParsePageObjectsView, LiteParsePathSegment, LiteParsePdfBounds,
};
use crate::raw_text::{LiteParseRawTextItem, LiteParseRawTextPage, LiteParseRawTextView};
use crate::records::*;
use crate::render::{
    LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION, LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE,
};
use crate::result::LiteParseResultView;
use crate::screenshots::LiteParseScreenshotsView;
use crate::{LiteParseComplexityView, LiteParsePageComplexity};

/// FNV-1a over the generated header with `\r` removed, so every checkout
/// yields the same value. CI keeps the header identical to the compiled
/// definitions, so the header determines every exported layout.
const fn header_fingerprint(header: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut index = 0;
    while index < header.len() {
        if header[index] != b'\r' {
            hash = (hash ^ header[index] as u64).wrapping_mul(0x100000001b3);
        }
        index += 1;
    }
    hash
}

const LAYOUT_FINGERPRINT: u64 = header_fingerprint(include_bytes!("../include/liteparse.h"));

// Every result view starts with its arenas, so one routine copies them.
const _: () = {
    use std::mem::offset_of;
    assert!(offset_of!(crate::content::LiteParseContentArrays, arenas) == 0);
    assert!(offset_of!(LiteParseResultView, content) == 0);
    assert!(offset_of!(LiteParseScreenshotsView, arenas) == 0);
    assert!(offset_of!(LiteParseComplexityView, arenas) == 0);
    assert!(offset_of!(LiteParseRawTextView, arenas) == 0);
    assert!(offset_of!(LiteParsePageObjectsView, arenas) == 0);
    assert!(offset_of!(LiteParseOcrRasterView, arenas) == 0);
};

/// Capability flags reported by `liteparse_abi_descriptor`.
pub const LITEPARSE_CAPABILITY_TESSERACT: u32 = 1;

/// Runtime layout and compiled-feature descriptor. `layout_fingerprint`
/// equals `LITEPARSE_ABI_FINGERPRINT` from the header the library was built
/// with.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseAbiDescriptor {
    pub pointer_width: u32,
    /// 0 for little endian, 1 for big endian.
    pub endianness: u32,
    pub capabilities: u32,
    pub layout_fingerprint: u64,
    pub max_selected_pages: u32,
    pub max_content_input_bytes: u64,
    pub max_single_binary_bytes: u64,
    pub max_content_binary_bytes: u64,
    pub max_result_bytes: u64,
    pub max_raster_pixels_per_page: u64,
    pub max_raster_bytes_per_operation: u64,
    pub max_objects: u32,
    pub max_object_nesting_depth: u32,
}

/// Write the library's ABI descriptor. Null is a no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_abi_descriptor(out: *mut LiteParseAbiDescriptor) {
    unsafe {
        write_out(
            out,
            LiteParseAbiDescriptor {
                pointer_width: usize::BITS,
                endianness: u32::from(cfg!(target_endian = "big")),
                capabilities: if cfg!(feature = "tesseract") {
                    LITEPARSE_CAPABILITY_TESSERACT
                } else {
                    0
                },
                layout_fingerprint: LAYOUT_FINGERPRINT,
                max_selected_pages: LITEPARSE_MAX_SELECTED_PAGES,
                max_content_input_bytes: LITEPARSE_MAX_CONTENT_INPUT_BYTES,
                max_single_binary_bytes: LITEPARSE_MAX_SINGLE_BINARY_BYTES,
                max_content_binary_bytes: LITEPARSE_MAX_CONTENT_BINARY_BYTES,
                max_result_bytes: LITEPARSE_MAX_RESULT_BYTES,
                max_raster_pixels_per_page: LITEPARSE_MAX_RASTER_PIXELS_PER_PAGE,
                max_raster_bytes_per_operation: LITEPARSE_MAX_RASTER_BYTES_PER_OPERATION,
                max_objects: LITEPARSE_MAX_OBJECTS,
                max_object_nesting_depth: LITEPARSE_MAX_OBJECT_NESTING_DEPTH,
            },
        )
    };
}

// cbindgen does not expand macros, so the selectors and the match are
// spelled out.
/// `liteparse_sizeof` selector for `LiteParseByteView`.
pub const LITEPARSE_TYPE_BYTE_VIEW: u32 = 0;
/// `liteparse_sizeof` selector for `LiteParseStr`.
pub const LITEPARSE_TYPE_STR: u32 = 1;
/// `liteparse_sizeof` selector for `LiteParseRect`.
pub const LITEPARSE_TYPE_RECT: u32 = 2;
/// `liteparse_sizeof` selector for `LiteParsePageGeometry`.
pub const LITEPARSE_TYPE_PAGE_GEOMETRY: u32 = 3;
/// `liteparse_sizeof` selector for `LiteParsePageComplexity`.
pub const LITEPARSE_TYPE_PAGE_COMPLEXITY: u32 = 4;
/// `liteparse_sizeof` selector for `LiteParseTextItem`.
pub const LITEPARSE_TYPE_TEXT_ITEM: u32 = 5;
/// `liteparse_sizeof` selector for `LiteParseWordBox`.
pub const LITEPARSE_TYPE_WORD_BOX: u32 = 6;
/// `liteparse_sizeof` selector for `LiteParseGraphic`.
pub const LITEPARSE_TYPE_GRAPHIC: u32 = 7;
/// `liteparse_sizeof` selector for `LiteParseStructNode`.
pub const LITEPARSE_TYPE_STRUCT_NODE: u32 = 8;
/// `liteparse_sizeof` selector for `LiteParseImageRef`.
pub const LITEPARSE_TYPE_IMAGE_REF: u32 = 9;
/// `liteparse_sizeof` selector for `LiteParseImage`.
pub const LITEPARSE_TYPE_IMAGE: u32 = 10;
/// `liteparse_sizeof` selector for `LiteParseAnnotation`.
pub const LITEPARSE_TYPE_ANNOTATION: u32 = 11;
/// `liteparse_sizeof` selector for `LiteParseFormField`.
pub const LITEPARSE_TYPE_FORM_FIELD: u32 = 12;
/// `liteparse_sizeof` selector for `LiteParseStructureNode`.
pub const LITEPARSE_TYPE_STRUCTURE_NODE: u32 = 13;
/// `liteparse_sizeof` selector for `LiteParseStructureAttribute`.
pub const LITEPARSE_TYPE_STRUCTURE_ATTRIBUTE: u32 = 14;
/// `liteparse_sizeof` selector for `LiteParseLayoutBlock`.
pub const LITEPARSE_TYPE_LAYOUT_BLOCK: u32 = 15;
/// `liteparse_sizeof` selector for `LiteParseLayoutCell`.
pub const LITEPARSE_TYPE_LAYOUT_CELL: u32 = 16;
/// `liteparse_sizeof` selector for `LiteParseLayoutRow`.
pub const LITEPARSE_TYPE_LAYOUT_ROW: u32 = 17;
/// `liteparse_sizeof` selector for `LiteParseVectorShape`.
pub const LITEPARSE_TYPE_VECTOR_SHAPE: u32 = 18;
/// `liteparse_sizeof` selector for `LiteParseVectorLine`.
pub const LITEPARSE_TYPE_VECTOR_LINE: u32 = 19;
/// `liteparse_sizeof` selector for `LiteParseOutlineEntry`.
pub const LITEPARSE_TYPE_OUTLINE_ENTRY: u32 = 20;
/// `liteparse_sizeof` selector for `LiteParsePageError`.
pub const LITEPARSE_TYPE_PAGE_ERROR: u32 = 21;
/// `liteparse_sizeof` selector for `LiteParseXfaPacket`.
pub const LITEPARSE_TYPE_XFA_PACKET: u32 = 22;
/// `liteparse_sizeof` selector for `LiteParseDocumentMeta`.
pub const LITEPARSE_TYPE_DOCUMENT_META: u32 = 23;
/// `liteparse_sizeof` selector for `LiteParseScreenshot`.
pub const LITEPARSE_TYPE_SCREENSHOT: u32 = 24;
/// `liteparse_sizeof` selector for `LiteParseScreenshotRect`.
pub const LITEPARSE_TYPE_SCREENSHOT_RECT: u32 = 25;
/// `liteparse_sizeof` selector for `LiteParseProjectedLine`.
pub const LITEPARSE_TYPE_PROJECTED_LINE: u32 = 26;
/// `liteparse_sizeof` selector for `LiteParseProjectedRegion`.
pub const LITEPARSE_TYPE_PROJECTED_REGION: u32 = 27;
/// `liteparse_sizeof` selector for `LiteParseItemFrame`.
pub const LITEPARSE_TYPE_ITEM_FRAME: u32 = 28;
/// `liteparse_sizeof` selector for `LiteParsePage`.
pub const LITEPARSE_TYPE_PAGE: u32 = 29;
/// `liteparse_sizeof` selector for `LiteParseContentInput`.
pub const LITEPARSE_TYPE_CONTENT_INPUT: u32 = 30;
/// `liteparse_sizeof` selector for `LiteParseResultView`.
pub const LITEPARSE_TYPE_RESULT_VIEW: u32 = 31;
/// `liteparse_sizeof` selector for `LiteParseArenas`.
pub const LITEPARSE_TYPE_ARENAS: u32 = 32;
/// `liteparse_sizeof` selector for `LiteParseScreenshotsView`.
pub const LITEPARSE_TYPE_SCREENSHOTS_VIEW: u32 = 33;
/// `liteparse_sizeof` selector for `LiteParseComplexityView`.
pub const LITEPARSE_TYPE_COMPLEXITY_VIEW: u32 = 34;
/// `liteparse_sizeof` selector for `LiteParseDocumentInfo`.
pub const LITEPARSE_TYPE_DOCUMENT_INFO: u32 = 35;
/// `liteparse_sizeof` selector for `LiteParseRenderRegion`.
pub const LITEPARSE_TYPE_RENDER_REGION: u32 = 36;
/// `liteparse_sizeof` selector for `LiteParseConfig`.
pub const LITEPARSE_TYPE_CONFIG: u32 = 37;
/// `liteparse_sizeof` selector for `LiteParseHeader`.
pub const LITEPARSE_TYPE_HEADER: u32 = 38;
/// `liteparse_sizeof` selector for `LiteParsePageOrientationCorrection`.
pub const LITEPARSE_TYPE_PAGE_ORIENTATION_CORRECTION: u32 = 39;
/// `liteparse_sizeof` selector for `LiteParseOcrRaster`.
pub const LITEPARSE_TYPE_OCR_RASTER: u32 = 40;
/// `liteparse_sizeof` selector for `LiteParseOcrWord`.
pub const LITEPARSE_TYPE_OCR_WORD: u32 = 41;
/// `liteparse_sizeof` selector for `LiteParsePageObjectPage`.
pub const LITEPARSE_TYPE_PAGE_OBJECT_PAGE: u32 = 42;
/// `liteparse_sizeof` selector for `LiteParseMatrix`.
pub const LITEPARSE_TYPE_MATRIX: u32 = 43;
/// `liteparse_sizeof` selector for `LiteParsePdfBounds`.
pub const LITEPARSE_TYPE_PDF_BOUNDS: u32 = 44;
/// `liteparse_sizeof` selector for `LiteParsePageObject`.
pub const LITEPARSE_TYPE_PAGE_OBJECT: u32 = 45;
/// `liteparse_sizeof` selector for `LiteParsePathSegment`.
pub const LITEPARSE_TYPE_PATH_SEGMENT: u32 = 46;
/// `liteparse_sizeof` selector for `LiteParsePageObjectsView`.
pub const LITEPARSE_TYPE_PAGE_OBJECTS_VIEW: u32 = 47;
/// `liteparse_sizeof` selector for `LiteParseRawTextPage`.
pub const LITEPARSE_TYPE_RAW_TEXT_PAGE: u32 = 48;
/// `liteparse_sizeof` selector for `LiteParseRawTextItem`.
pub const LITEPARSE_TYPE_RAW_TEXT_ITEM: u32 = 49;
/// `liteparse_sizeof` selector for `LiteParseRawTextView`.
pub const LITEPARSE_TYPE_RAW_TEXT_VIEW: u32 = 50;
/// `liteparse_sizeof` selector for `LiteParseContentArrays`.
pub const LITEPARSE_TYPE_CONTENT_ARRAYS: u32 = 51;
/// `liteparse_sizeof` selector for `LiteParsePageOutput`.
pub const LITEPARSE_TYPE_PAGE_OUTPUT: u32 = 52;
/// `liteparse_sizeof` selector for `LiteParseBlobRef`.
pub const LITEPARSE_TYPE_BLOB_REF: u32 = 53;
/// `liteparse_sizeof` selector for `LiteParseAbiDescriptor`.
pub const LITEPARSE_TYPE_ABI_DESCRIPTOR: u32 = 54;
/// `liteparse_sizeof` selector for `LiteParseOcrPageInput`.
pub const LITEPARSE_TYPE_OCR_PAGE_INPUT: u32 = 55;
/// `liteparse_sizeof` selector for `LiteParseOcrRasterView`.
pub const LITEPARSE_TYPE_OCR_RASTER_VIEW: u32 = 56;
/// `liteparse_sizeof` selector for `LiteParseClipPath`.
pub const LITEPARSE_TYPE_CLIP_PATH: u32 = 57;
/// `liteparse_sizeof` selector for `LiteParseDocumentMetadata`.
pub const LITEPARSE_TYPE_DOCUMENT_METADATA: u32 = 58;

/// `sizeof` the struct a `LITEPARSE_TYPE_*` selector names, or zero for an
/// unknown selector. Lets a binding that declares layouts by hand assert
/// them against the library it loaded.
#[unsafe(no_mangle)]
pub extern "C" fn liteparse_sizeof(type_id: u32) -> usize {
    match type_id {
        LITEPARSE_TYPE_BYTE_VIEW => size_of::<LiteParseByteView>(),
        LITEPARSE_TYPE_STR => size_of::<LiteParseStr>(),
        LITEPARSE_TYPE_RECT => size_of::<LiteParseRect>(),
        LITEPARSE_TYPE_PAGE_GEOMETRY => size_of::<LiteParsePageGeometry>(),
        LITEPARSE_TYPE_PAGE_COMPLEXITY => size_of::<LiteParsePageComplexity>(),
        LITEPARSE_TYPE_TEXT_ITEM => size_of::<LiteParseTextItem>(),
        LITEPARSE_TYPE_WORD_BOX => size_of::<LiteParseWordBox>(),
        LITEPARSE_TYPE_GRAPHIC => size_of::<LiteParseGraphic>(),
        LITEPARSE_TYPE_STRUCT_NODE => size_of::<LiteParseStructNode>(),
        LITEPARSE_TYPE_IMAGE_REF => size_of::<LiteParseImageRef>(),
        LITEPARSE_TYPE_IMAGE => size_of::<LiteParseImage>(),
        LITEPARSE_TYPE_ANNOTATION => size_of::<LiteParseAnnotation>(),
        LITEPARSE_TYPE_FORM_FIELD => size_of::<LiteParseFormField>(),
        LITEPARSE_TYPE_STRUCTURE_NODE => size_of::<LiteParseStructureNode>(),
        LITEPARSE_TYPE_STRUCTURE_ATTRIBUTE => size_of::<LiteParseStructureAttribute>(),
        LITEPARSE_TYPE_LAYOUT_BLOCK => size_of::<LiteParseLayoutBlock>(),
        LITEPARSE_TYPE_LAYOUT_CELL => size_of::<LiteParseLayoutCell>(),
        LITEPARSE_TYPE_LAYOUT_ROW => size_of::<LiteParseLayoutRow>(),
        LITEPARSE_TYPE_VECTOR_SHAPE => size_of::<LiteParseVectorShape>(),
        LITEPARSE_TYPE_VECTOR_LINE => size_of::<LiteParseVectorLine>(),
        LITEPARSE_TYPE_OUTLINE_ENTRY => size_of::<LiteParseOutlineEntry>(),
        LITEPARSE_TYPE_PAGE_ERROR => size_of::<LiteParsePageError>(),
        LITEPARSE_TYPE_XFA_PACKET => size_of::<LiteParseXfaPacket>(),
        LITEPARSE_TYPE_DOCUMENT_META => size_of::<LiteParseDocumentMeta>(),
        LITEPARSE_TYPE_SCREENSHOT => size_of::<LiteParseScreenshot>(),
        LITEPARSE_TYPE_SCREENSHOT_RECT => size_of::<LiteParseScreenshotRect>(),
        LITEPARSE_TYPE_PROJECTED_LINE => size_of::<LiteParseProjectedLine>(),
        LITEPARSE_TYPE_PROJECTED_REGION => size_of::<LiteParseProjectedRegion>(),
        LITEPARSE_TYPE_ITEM_FRAME => size_of::<LiteParseItemFrame>(),
        LITEPARSE_TYPE_PAGE => size_of::<LiteParsePage>(),
        LITEPARSE_TYPE_CONTENT_INPUT => size_of::<LiteParseContentInput>(),
        LITEPARSE_TYPE_CONTENT_ARRAYS => size_of::<crate::content::LiteParseContentArrays>(),
        LITEPARSE_TYPE_PAGE_OUTPUT => size_of::<LiteParsePageOutput>(),
        LITEPARSE_TYPE_BLOB_REF => size_of::<crate::handle::LiteParseBlobRef>(),
        LITEPARSE_TYPE_ABI_DESCRIPTOR => size_of::<LiteParseAbiDescriptor>(),
        LITEPARSE_TYPE_RESULT_VIEW => size_of::<LiteParseResultView>(),
        LITEPARSE_TYPE_ARENAS => size_of::<LiteParseArenas>(),
        LITEPARSE_TYPE_SCREENSHOTS_VIEW => size_of::<LiteParseScreenshotsView>(),
        LITEPARSE_TYPE_COMPLEXITY_VIEW => size_of::<LiteParseComplexityView>(),
        LITEPARSE_TYPE_DOCUMENT_INFO => size_of::<LiteParseDocumentInfo>(),
        LITEPARSE_TYPE_DOCUMENT_METADATA => size_of::<LiteParseDocumentMetadata>(),
        LITEPARSE_TYPE_RENDER_REGION => size_of::<LiteParseRenderRegion>(),
        LITEPARSE_TYPE_CONFIG => size_of::<LiteParseConfig>(),
        LITEPARSE_TYPE_HEADER => size_of::<LiteParseHeader>(),
        LITEPARSE_TYPE_PAGE_ORIENTATION_CORRECTION => {
            size_of::<LiteParsePageOrientationCorrection>()
        }
        LITEPARSE_TYPE_OCR_RASTER => size_of::<LiteParseOcrRaster>(),
        LITEPARSE_TYPE_OCR_PAGE_INPUT => size_of::<LiteParseOcrPageInput>(),
        LITEPARSE_TYPE_OCR_RASTER_VIEW => size_of::<LiteParseOcrRasterView>(),
        LITEPARSE_TYPE_OCR_WORD => size_of::<LiteParseOcrWord>(),
        LITEPARSE_TYPE_PAGE_OBJECT_PAGE => size_of::<LiteParsePageObjectPage>(),
        LITEPARSE_TYPE_MATRIX => size_of::<LiteParseMatrix>(),
        LITEPARSE_TYPE_PDF_BOUNDS => size_of::<LiteParsePdfBounds>(),
        LITEPARSE_TYPE_PAGE_OBJECT => size_of::<LiteParsePageObject>(),
        LITEPARSE_TYPE_PATH_SEGMENT => size_of::<LiteParsePathSegment>(),
        LITEPARSE_TYPE_CLIP_PATH => size_of::<LiteParseClipPath>(),
        LITEPARSE_TYPE_PAGE_OBJECTS_VIEW => size_of::<LiteParsePageObjectsView>(),
        LITEPARSE_TYPE_RAW_TEXT_PAGE => size_of::<LiteParseRawTextPage>(),
        LITEPARSE_TYPE_RAW_TEXT_ITEM => size_of::<LiteParseRawTextItem>(),
        LITEPARSE_TYPE_RAW_TEXT_VIEW => size_of::<LiteParseRawTextView>(),
        _ => 0,
    }
}

#[cfg(test)]
const ABI_TYPE_IDS: &[u32] = &[
    LITEPARSE_TYPE_BYTE_VIEW,
    LITEPARSE_TYPE_STR,
    LITEPARSE_TYPE_RECT,
    LITEPARSE_TYPE_PAGE_GEOMETRY,
    LITEPARSE_TYPE_PAGE_COMPLEXITY,
    LITEPARSE_TYPE_TEXT_ITEM,
    LITEPARSE_TYPE_WORD_BOX,
    LITEPARSE_TYPE_GRAPHIC,
    LITEPARSE_TYPE_STRUCT_NODE,
    LITEPARSE_TYPE_IMAGE_REF,
    LITEPARSE_TYPE_IMAGE,
    LITEPARSE_TYPE_ANNOTATION,
    LITEPARSE_TYPE_FORM_FIELD,
    LITEPARSE_TYPE_STRUCTURE_NODE,
    LITEPARSE_TYPE_STRUCTURE_ATTRIBUTE,
    LITEPARSE_TYPE_LAYOUT_BLOCK,
    LITEPARSE_TYPE_LAYOUT_CELL,
    LITEPARSE_TYPE_LAYOUT_ROW,
    LITEPARSE_TYPE_VECTOR_SHAPE,
    LITEPARSE_TYPE_VECTOR_LINE,
    LITEPARSE_TYPE_OUTLINE_ENTRY,
    LITEPARSE_TYPE_PAGE_ERROR,
    LITEPARSE_TYPE_XFA_PACKET,
    LITEPARSE_TYPE_DOCUMENT_META,
    LITEPARSE_TYPE_SCREENSHOT,
    LITEPARSE_TYPE_SCREENSHOT_RECT,
    LITEPARSE_TYPE_PROJECTED_LINE,
    LITEPARSE_TYPE_PROJECTED_REGION,
    LITEPARSE_TYPE_ITEM_FRAME,
    LITEPARSE_TYPE_PAGE,
    LITEPARSE_TYPE_CONTENT_INPUT,
    LITEPARSE_TYPE_RESULT_VIEW,
    LITEPARSE_TYPE_ARENAS,
    LITEPARSE_TYPE_SCREENSHOTS_VIEW,
    LITEPARSE_TYPE_COMPLEXITY_VIEW,
    LITEPARSE_TYPE_DOCUMENT_INFO,
    LITEPARSE_TYPE_DOCUMENT_METADATA,
    LITEPARSE_TYPE_RENDER_REGION,
    LITEPARSE_TYPE_CONFIG,
    LITEPARSE_TYPE_HEADER,
    LITEPARSE_TYPE_PAGE_ORIENTATION_CORRECTION,
    LITEPARSE_TYPE_OCR_RASTER,
    LITEPARSE_TYPE_OCR_PAGE_INPUT,
    LITEPARSE_TYPE_OCR_RASTER_VIEW,
    LITEPARSE_TYPE_OCR_WORD,
    LITEPARSE_TYPE_PAGE_OBJECT_PAGE,
    LITEPARSE_TYPE_MATRIX,
    LITEPARSE_TYPE_PDF_BOUNDS,
    LITEPARSE_TYPE_PAGE_OBJECT,
    LITEPARSE_TYPE_PATH_SEGMENT,
    LITEPARSE_TYPE_CLIP_PATH,
    LITEPARSE_TYPE_PAGE_OBJECTS_VIEW,
    LITEPARSE_TYPE_RAW_TEXT_PAGE,
    LITEPARSE_TYPE_RAW_TEXT_ITEM,
    LITEPARSE_TYPE_RAW_TEXT_VIEW,
    LITEPARSE_TYPE_CONTENT_ARRAYS,
    LITEPARSE_TYPE_PAGE_OUTPUT,
    LITEPARSE_TYPE_BLOB_REF,
    LITEPARSE_TYPE_ABI_DESCRIPTOR,
];

#[cfg(test)]
mod tests {
    use super::*;

    const ABI_HEADER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/include/liteparse_abi.h");

    fn abi_header() -> String {
        format!(
            "/* Generated by the liteparse-c tests; rewrite with LITEPARSE_BLESS=1. */\n\
             #ifndef LITEPARSE_ABI_H\n\
             #define LITEPARSE_ABI_H\n\
             \n\
             /* Expected LiteParseAbiDescriptor.layout_fingerprint for liteparse.h. */\n\
             #define LITEPARSE_ABI_FINGERPRINT {LAYOUT_FINGERPRINT:#018x}ULL\n\
             \n\
             #endif\n"
        )
    }

    #[test]
    fn published_fingerprint_matches_the_header() {
        let expected = abi_header();
        if std::env::var_os("LITEPARSE_BLESS").is_some() {
            std::fs::write(ABI_HEADER, &expected).unwrap();
            return;
        }
        let actual = std::fs::read_to_string(ABI_HEADER).unwrap_or_default();
        assert_eq!(
            actual.replace("\r\n", "\n"),
            expected,
            "include/liteparse_abi.h is stale; rerun the tests with LITEPARSE_BLESS=1"
        );
    }

    #[test]
    fn every_selector_is_unique_and_nonzero_sized() {
        let mut seen = std::collections::BTreeSet::new();
        for id in ABI_TYPE_IDS {
            assert!(seen.insert(*id), "duplicate selector {id}");
            assert!(liteparse_sizeof(*id) > 0, "selector {id} has no size");
        }
        assert_eq!(liteparse_sizeof(u32::MAX), 0);
    }
}
