//! The file's revisions and signatures as PDFium reads them.
//!
//! PDFium records where each trailer dictionary of a cross-reference table
//! ends as it parses the file (`FPDF_GetTrailerEnds`): after the end-of-file
//! marker and the line end that follows it, and only where a line end does
//! follow it. The ends it states are therefore those of the revisions that
//! close with such a table and a line end; a file whose sections are
//! cross-reference streams states none, and a file whose last marker ends the
//! file without a line end states no end for its last revision. PDFium reads every
//! signature dictionary of the AcroForm with its `/ByteRange`, `/SubFilter`
//! and `/M`, but names neither its field nor its `/Type`.

use std::ffi::{c_char, c_int, c_uint, c_ulong};

use liteparse_pdfium::{Library, pdfium_sys};

use crate::document::{LiteParseDocument, Source};
use crate::handle::{LiteParseStr, Pool, array_ptr, required_out, state_ref, view_state};
use crate::pdfium_extra::extra;
use crate::raw::RawDocument;
use crate::status::{FfiResult, LiteParseStatus, boundary};

/// `LiteParseStatedRevisions.flags` bit: PDFium reported where the file's
/// trailers end, and `ends` holds those ends, oldest first.
pub const LITEPARSE_STATED_REVISIONS_FLAG_ENDS: u32 = 1 << 0;
/// Flag: PDFium reads signatures; when clear, `signatures` is empty because
/// the loaded PDFium has no signature support, not because the file has none.
pub const LITEPARSE_STATED_REVISIONS_FLAG_SIGNATURES: u32 = 1 << 1;

/// One signature dictionary. Strings index the view's pool; `range_offset` and
/// `range_count` range into the view's `ranges`.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseStatedSignature {
    /// `/SubFilter` without its slash.
    pub sub_filter: LiteParseStr,
    /// `/M` as written.
    pub time: LiteParseStr,
    pub range_offset: u32,
    pub range_count: u32,
}

/// What the file states about its revisions and signatures, as PDFium reads
/// them. Borrowed until the document is freed. Every field is empty for an
/// input that was converted to PDF.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseStatedRevisions {
    /// String pool behind `signatures`.
    pub pool: *const u8,
    pub pool_len: usize,
    /// The byte offsets at which the file's trailers end, oldest first.
    pub ends: *const u64,
    pub ends_len: usize,
    pub signatures: *const LiteParseStatedSignature,
    pub signatures_len: usize,
    /// The `/ByteRange` entries of every signature, in order.
    pub ranges: *const i64,
    pub ranges_len: usize,
    /// `LITEPARSE_STATED_REVISIONS_FLAG_*` bits.
    pub flags: u32,
}

/// The revisions packed once, with the storage behind their view.
pub(crate) struct StatedRevisionsState {
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    ends: Vec<u64>,
    #[allow(dead_code)]
    signatures: Vec<LiteParseStatedSignature>,
    #[allow(dead_code)]
    ranges: Vec<i64>,
    pub(crate) view: LiteParseStatedRevisions,
}

view_state!(StatedRevisionsState => LiteParseStatedRevisions, view);

/// The most trailer ends or signatures read; a file holding more states the
/// first ones.
const MAX_ENTRIES: c_ulong = 65_536;

/// Read the revisions and signatures of `source`'s input under the PDFium lock
/// `lib` holds.
pub(crate) fn read(source: &Source, lib: &Library) -> FfiResult<StatedRevisionsState> {
    let mut pool = Pool::default();
    let mut ends = Vec::new();
    let mut signatures = Vec::new();
    let mut ranges = Vec::new();
    let mut flags = 0;
    if !source.is_converted() {
        let extra = extra()?;
        let raw = RawDocument::open(
            lib,
            &source.input,
            source.config.password.as_deref(),
            "revisions",
        )?;
        let pdfium = pdfium_sys::dynamic::pdfium();
        // SAFETY: `raw` is open under the PDFium lock `lib` holds; every buffer
        // is as long as the length passed with it.
        unsafe {
            let count = (extra.trailer_ends)(raw.handle, std::ptr::null_mut(), 0).min(MAX_ENTRIES);
            if count > 0 {
                let mut buffer = vec![0 as c_uint; count as usize];
                let read = (extra.trailer_ends)(raw.handle, buffer.as_mut_ptr(), count);
                ends.extend(
                    buffer
                        .iter()
                        .take(read.min(count) as usize)
                        .map(|end| *end as u64),
                );
                flags |= LITEPARSE_STATED_REVISIONS_FLAG_ENDS;
            }
            if let (Some(count), Some(object), Some(range)) = (
                pdfium.FPDF_GetSignatureCount,
                pdfium.FPDF_GetSignatureObject,
                pdfium.FPDFSignatureObj_GetByteRange,
            ) {
                flags |= LITEPARSE_STATED_REVISIONS_FLAG_SIGNATURES;
                for index in 0..count(raw.handle).max(0).min(MAX_ENTRIES as c_int) {
                    let signature = object(raw.handle, index);
                    if signature.is_null() {
                        continue;
                    }
                    let length = range(signature, std::ptr::null_mut(), 0).min(MAX_ENTRIES);
                    let mut values = vec![0 as c_int; length as usize];
                    let read = range(signature, values.as_mut_ptr(), length).min(length);
                    let offset = ranges.len();
                    ranges.extend(values.iter().take(read as usize).map(|value| *value as i64));
                    signatures.push(LiteParseStatedSignature {
                        sub_filter: pool.push_opt(
                            ascii(|buffer, length| {
                                (extra.signature_sub_filter)(signature, buffer, length)
                            })
                            .as_deref(),
                        ),
                        time: pool.push_opt(
                            ascii(|buffer, length| {
                                (extra.signature_time)(signature, buffer, length)
                            })
                            .as_deref(),
                        ),
                        range_offset: offset as u32,
                        range_count: (ranges.len() - offset) as u32,
                    });
                }
            }
        }
    }
    let view = LiteParseStatedRevisions {
        pool: pool.ptr(),
        pool_len: pool.len(),
        ends: array_ptr(&ends),
        ends_len: ends.len(),
        signatures: array_ptr(&signatures),
        signatures_len: signatures.len(),
        ranges: array_ptr(&ranges),
        ranges_len: ranges.len(),
        flags,
    };
    Ok(StatedRevisionsState {
        pool,
        ends,
        signatures,
        ranges,
        view,
    })
}

/// A NUL-terminated ASCII string a PDFium getter writes, absent when it writes
/// none.
fn ascii(mut read: impl FnMut(*mut c_char, c_ulong) -> c_ulong) -> Option<String> {
    let length = read(std::ptr::null_mut(), 0);
    if length <= 1 {
        return None;
    }
    let mut buffer = vec![0 as c_char; length as usize];
    let written = read(buffer.as_mut_ptr(), length).min(length) as usize;
    let bytes: Vec<u8> = buffer[..written.saturating_sub(1)]
        .iter()
        .map(|byte| *byte as u8)
        .collect();
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Borrow the file's revisions and signatures as PDFium reads them (see
/// [`LiteParseStatedRevisions`]). The first call on a document reads and the
/// document keeps it; the view is valid until the document is freed.
///
/// `document` must be live and `out` writable; `out` receives null on
/// failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_stated_revisions(
    document: *const LiteParseDocument,
    out: *mut *const LiteParseStatedRevisions,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let out = required_out(out)?;
        out.as_ptr().write(std::ptr::null());
        out.as_ptr().write(state_ref(document)?.stated_revisions()?);
        Ok(())
    })
}
