//! The document information dictionary and the XMP packet, as stated.
//!
//! The metadata `liteparse_document_metadata` reports reads the nine standard
//! `/Info` keys through PDFium and caps the XMP at 64 KiB and the file at
//! 16 MiB. This reads the file's own objects instead, with the PDF reader the
//! core already depends on, so that:
//!
//! - every `/Info` entry is reported with its key, its value and the kind of
//!   value the file holds (a text string, a name, a number, a boolean or
//!   anything else), custom keys included;
//! - the catalog's `/Metadata` stream is reported whole, decoded by its
//!   encoding, whatever its size.
//!
//! The reader loads every object of the file, which on a file with many
//! objects costs more than parsing it, so nothing else calls it. When the
//! reader cannot load the file, the nine standard keys come from PDFium as
//! text and `LITEPARSE_STATED_METADATA_FLAG_INFO_PARTIAL` says the list may
//! be short.

use liteparse::types::PdfInput;
use liteparse_pdfium::Library;
use lopdf::{Dictionary, Document as Pdf, Object};

use crate::document::{LiteParseDocument, Source};
use crate::handle::{LiteParseStr, Pool, array_ptr, required_out, state_ref, view_state};
use crate::status::{FfiResult, LiteParseStatus, boundary};

/// `LiteParseInfoEntry.kind`: a text string.
pub const LITEPARSE_INFO_KIND_TEXT: u32 = 0;
/// A name; the value is the name without its slash.
pub const LITEPARSE_INFO_KIND_NAME: u32 = 1;
/// An integer or a real number; the value is its decimal text.
pub const LITEPARSE_INFO_KIND_NUMBER: u32 = 2;
/// A boolean; the value is `true` or `false`.
pub const LITEPARSE_INFO_KIND_BOOLEAN: u32 = 3;
/// Anything else (null, array, dictionary); the value is its PDF syntax.
pub const LITEPARSE_INFO_KIND_OTHER: u32 = 4;

/// `LiteParseStatedMetadata.flags` bit: the trailer states an information
/// dictionary (it may be empty).
pub const LITEPARSE_STATED_METADATA_FLAG_HAS_INFO: u32 = 1 << 0;
/// Flag: the catalog states an XMP stream and `xmp` holds it.
pub const LITEPARSE_STATED_METADATA_FLAG_HAS_XMP: u32 = 1 << 1;
/// Flag: the catalog states an XMP stream this library cannot decode (an
/// unsupported filter); `xmp` is empty.
pub const LITEPARSE_STATED_METADATA_FLAG_XMP_UNREADABLE: u32 = 1 << 2;
/// Flag: the file's objects could not be read, so `entries` holds only the
/// standard keys PDFium returned and `xmp` is empty.
pub const LITEPARSE_STATED_METADATA_FLAG_INFO_PARTIAL: u32 = 1 << 3;
/// Flag: `language` holds the catalog's `/Lang` as written.
pub const LITEPARSE_STATED_METADATA_FLAG_HAS_LANGUAGE: u32 = 1 << 4;

/// One `/Info` entry. Strings index the metadata's pool.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LiteParseInfoEntry {
    /// The key without its slash.
    pub key: LiteParseStr,
    pub value: LiteParseStr,
    /// `LITEPARSE_INFO_KIND_*`.
    pub kind: u32,
}

/// What the file states about itself: the information dictionary in file
/// order and the XMP packet. Borrowed until the document is freed. Every field
/// is absent for an input that was converted to PDF.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LiteParseStatedMetadata {
    /// String pool behind `entries` and `xmp`.
    pub pool: *const u8,
    pub pool_len: usize,
    pub entries: *const LiteParseInfoEntry,
    pub entries_len: usize,
    /// The XMP packet as text, decoded by its encoding (UTF-8, or UTF-16 by
    /// byte order mark or by the first bytes), byte order mark dropped.
    pub xmp: LiteParseStr,
    /// `LITEPARSE_STATED_METADATA_FLAG_*` bits.
    pub flags: u32,
    /// The catalog's `/Lang`, the document's language, when
    /// `FLAG_HAS_LANGUAGE` is set.
    pub language: LiteParseStr,
}

/// The metadata packed once, with the storage behind its view.
pub(crate) struct StatedMetadataState {
    #[allow(dead_code)]
    pool: Pool,
    #[allow(dead_code)]
    entries: Vec<LiteParseInfoEntry>,
    pub(crate) view: LiteParseStatedMetadata,
}

view_state!(StatedMetadataState => LiteParseStatedMetadata, view);

/// The standard keys PDFium can read when the file's objects cannot be.
const STANDARD_KEYS: [&str; 9] = [
    "Title",
    "Author",
    "Subject",
    "Keywords",
    "Creator",
    "Producer",
    "Trapped",
    "CreationDate",
    "ModDate",
];

/// The longest PDF syntax kept for a value of kind `OTHER`.
const MAX_SYNTAX_DEPTH: usize = 8;

/// Read the metadata of `source`'s input. `lib` is the PDFium lock, held for
/// the fallback read.
pub(crate) fn read(source: &Source, lib: &Library) -> FfiResult<StatedMetadataState> {
    let mut pool = Pool::default();
    let mut entries = Vec::new();
    let mut xmp = LiteParseStr::default();
    let mut language = LiteParseStr::default();
    let mut flags = 0;
    if !source.is_converted() {
        match load(&source.input, source.config.password.as_deref())
            .and_then(|pdf| stated(&pdf, &mut pool))
        {
            Some(read) => {
                flags |= read.0;
                entries = read.1;
                xmp = read.2;
                language = read.3;
            }
            None => {
                flags |= LITEPARSE_STATED_METADATA_FLAG_INFO_PARTIAL;
                let document = source.open(lib, &source.input)?;
                for key in STANDARD_KEYS {
                    // PDFium answers an absent key as an empty value.
                    if let Some(value) = document.meta_text(key).filter(|v| !v.is_empty()) {
                        entries.push(LiteParseInfoEntry {
                            key: pool.push(key),
                            value: pool.push(&value),
                            kind: LITEPARSE_INFO_KIND_TEXT,
                        });
                    }
                }
            }
        }
    }
    let view = LiteParseStatedMetadata {
        pool: pool.ptr(),
        pool_len: pool.len(),
        entries: array_ptr(&entries),
        entries_len: entries.len(),
        xmp,
        flags,
        language,
    };
    Ok(StatedMetadataState {
        pool,
        entries,
        view,
    })
}

/// The entries and packet of a loaded file, packed into `pool`, with the flags
/// they contribute; `None` when the file's objects are not all there. The
/// reader loads a damaged file without complaint and leaves out the objects it
/// could not parse, so a trailer or catalog that names an object the reader
/// lacks says the whole read cannot be trusted.
fn stated(
    pdf: &Pdf,
    pool: &mut Pool,
) -> Option<(u32, Vec<LiteParseInfoEntry>, LiteParseStr, LiteParseStr)> {
    let mut entries = Vec::new();
    let mut xmp = LiteParseStr::default();
    let mut language = LiteParseStr::default();
    let flags = info(pdf, pool, &mut entries)?
        | packet(pdf, pool, &mut xmp)?
        | catalog_language(pdf, pool, &mut language);
    Some((flags, entries, xmp, language))
}

/// Pack the catalog's `/Lang` as written; the flag it contributes.
fn catalog_language(pdf: &Pdf, pool: &mut Pool, language: &mut LiteParseStr) -> u32 {
    let Some(value) = pdf
        .catalog()
        .ok()
        .and_then(|catalog| catalog.get(b"Lang").ok())
    else {
        return 0;
    };
    let value = pdf.dereference(value).map_or(value, |(_, value)| value);
    if !matches!(value, Object::String(..)) {
        return 0;
    }
    *language = pool.push(&describe(pdf, value).1);
    LITEPARSE_STATED_METADATA_FLAG_HAS_LANGUAGE
}

/// The file's objects, decrypted when the file is encrypted; `None` when the
/// reader cannot load it or cannot open its encryption.
pub(crate) fn load(input: &PdfInput, password: Option<&str>) -> Option<Pdf> {
    let mut pdf = match input {
        PdfInput::Path(path) => Pdf::load(path).ok()?,
        PdfInput::Bytes(bytes) => Pdf::load_mem(bytes).ok()?,
    };
    if pdf.is_encrypted() {
        pdf.decrypt(password.unwrap_or("")).ok()?;
    }
    Some(pdf)
}

/// Pack the trailer's `/Info` entries; the flags it contributes, `None` when
/// the trailer names an information dictionary the reader lacks.
fn info(pdf: &Pdf, pool: &mut Pool, entries: &mut Vec<LiteParseInfoEntry>) -> Option<u32> {
    let Ok(info) = pdf.trailer.get(b"Info") else {
        return Some(0);
    };
    let dictionary = pdf.dereference(info).ok()?.1.as_dict().ok()?;
    for (key, value) in dictionary.iter() {
        let value = pdf.dereference(value).map_or(value, |(_, value)| value);
        let (kind, text) = describe(pdf, value);
        entries.push(LiteParseInfoEntry {
            key: pool.push(&String::from_utf8_lossy(key)),
            value: pool.push(&text),
            kind,
        });
    }
    Some(LITEPARSE_STATED_METADATA_FLAG_HAS_INFO)
}

/// The kind and text of an information value.
fn describe(pdf: &Pdf, value: &Object) -> (u32, String) {
    match value {
        Object::String(..) => (
            LITEPARSE_INFO_KIND_TEXT,
            lopdf::decode_text_string(value)
                .map(|text| text.trim_start_matches('\u{feff}').to_owned())
                .unwrap_or_else(|_| {
                    String::from_utf8_lossy(value.as_str().unwrap_or_default()).into_owned()
                }),
        ),
        Object::Name(name) => (
            LITEPARSE_INFO_KIND_NAME,
            String::from_utf8_lossy(name).into_owned(),
        ),
        Object::Integer(number) => (LITEPARSE_INFO_KIND_NUMBER, number.to_string()),
        Object::Real(number) => (LITEPARSE_INFO_KIND_NUMBER, number.to_string()),
        Object::Boolean(flag) => (LITEPARSE_INFO_KIND_BOOLEAN, flag.to_string()),
        other => (LITEPARSE_INFO_KIND_OTHER, syntax(pdf, other, 0)),
    }
}

/// PDF syntax for a value that is not a string, name, number or boolean.
/// References are written, not followed, and nesting deeper than
/// `MAX_SYNTAX_DEPTH` is elided.
fn syntax(pdf: &Pdf, value: &Object, depth: usize) -> String {
    if depth > MAX_SYNTAX_DEPTH {
        return "...".to_owned();
    }
    match value {
        Object::Null => "null".to_owned(),
        Object::Boolean(flag) => flag.to_string(),
        Object::Integer(number) => number.to_string(),
        Object::Real(number) => number.to_string(),
        Object::Name(name) => format!("/{}", String::from_utf8_lossy(name)),
        Object::String(..) => format!(
            "({})",
            lopdf::decode_text_string(value)
                .unwrap_or_else(
                    |_| String::from_utf8_lossy(value.as_str().unwrap_or_default()).into_owned()
                )
        ),
        Object::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| syntax(pdf, item, depth + 1))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Object::Dictionary(dictionary) => dictionary_syntax(pdf, dictionary, depth),
        Object::Stream(stream) => dictionary_syntax(pdf, &stream.dict, depth),
        Object::Reference((number, generation)) => format!("{number} {generation} R"),
    }
}

fn dictionary_syntax(pdf: &Pdf, dictionary: &Dictionary, depth: usize) -> String {
    let entries = dictionary
        .iter()
        .map(|(key, value)| {
            format!(
                "/{} {}",
                String::from_utf8_lossy(key),
                syntax(pdf, value, depth + 1)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("<< {entries} >>")
}

/// Pack the catalog's XMP stream; the flags it contributes, `None` when the
/// reader lacks the catalog or the stream it names.
fn packet(pdf: &Pdf, pool: &mut Pool, xmp: &mut LiteParseStr) -> Option<u32> {
    let Ok(metadata) = pdf.catalog().ok()?.get(b"Metadata") else {
        return Some(0);
    };
    // A catalog that states `/Metadata null` states none.
    let (_, object) = pdf.dereference(metadata).ok()?;
    if object.is_null() {
        return Some(0);
    }
    let stream = object.as_stream().ok()?;
    let bytes = if stream.dict.has(b"Filter") {
        match stream.decompressed_content() {
            Ok(bytes) => bytes,
            Err(_) => return Some(LITEPARSE_STATED_METADATA_FLAG_XMP_UNREADABLE),
        }
    } else {
        stream.content.clone()
    };
    *xmp = pool.push(&decode_packet(&bytes));
    Some(LITEPARSE_STATED_METADATA_FLAG_HAS_XMP)
}

/// An XMP packet as text. XMP is UTF-8 or UTF-16 in either byte order: a byte
/// order mark says which, and without one the first characters (`<?xpacket`
/// or `<x:xmpmeta`, ASCII) leave a zero byte in the half of the first code unit
/// that gives the order. The mark is dropped.
fn decode_packet(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], big_endian: bool| {
        let (pairs, _) = bytes.as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|pair| {
                if big_endian {
                    u16::from_be_bytes(*pair)
                } else {
                    u16::from_le_bytes(*pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, true),
        [0xFF, 0xFE, rest @ ..] => utf16(rest, false),
        [0, second, ..] if *second != 0 => utf16(bytes, true),
        [first, 0, ..] if *first != 0 => utf16(bytes, false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Borrow what the file states about itself: every `/Info` entry with its kind
/// of value, and the whole XMP packet (see [`LiteParseStatedMetadata`]). The
/// first call on a document reads and the document keeps it; the view is valid
/// until the document is freed. The read loads every object of the file, so it
/// can take seconds on a file with many; it is separate from
/// `liteparse_document_metadata`, which stays cheap.
///
/// `document` must be live and `out` writable; `out` receives null on
/// failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn liteparse_document_stated_metadata(
    document: *const LiteParseDocument,
    out: *mut *const LiteParseStatedMetadata,
) -> LiteParseStatus {
    boundary(|| unsafe {
        let out = required_out(out)?;
        out.as_ptr().write(std::ptr::null());
        out.as_ptr().write(state_ref(document)?.stated_metadata()?);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packet_is_decoded_by_its_byte_order_mark_or_its_first_characters() {
        let text = "<?xpacket begin=\"\"?><x:xmpmeta>\u{e9}</x:xmpmeta>";
        let utf8 = text.as_bytes().to_vec();
        let with = |mark: &[u8], body: Vec<u8>| [mark, &body].concat();
        let be: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let le: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_packet(&utf8), text);
        assert_eq!(
            decode_packet(&with(&[0xEF, 0xBB, 0xBF], utf8.clone())),
            text
        );
        assert_eq!(decode_packet(&with(&[0xFE, 0xFF], be.clone())), text);
        assert_eq!(decode_packet(&with(&[0xFF, 0xFE], le.clone())), text);
        assert_eq!(decode_packet(&be), text);
        assert_eq!(decode_packet(&le), text);
        assert_eq!(decode_packet(b""), "");
    }

    #[test]
    fn values_that_are_not_text_keep_their_kind() {
        let pdf = Pdf::new();
        let kind = |object: Object| describe(&pdf, &object);
        assert_eq!(
            kind(Object::Name(b"True".to_vec())),
            (LITEPARSE_INFO_KIND_NAME, "True".to_owned())
        );
        assert_eq!(
            kind(Object::Integer(7)),
            (LITEPARSE_INFO_KIND_NUMBER, "7".to_owned())
        );
        assert_eq!(
            kind(Object::Real(1.5)),
            (LITEPARSE_INFO_KIND_NUMBER, "1.5".to_owned())
        );
        assert_eq!(
            kind(Object::Boolean(true)),
            (LITEPARSE_INFO_KIND_BOOLEAN, "true".to_owned())
        );
        assert_eq!(
            kind(Object::Array(vec![
                Object::Integer(1),
                Object::Name(b"A".to_vec()),
                Object::Reference((4, 0)),
            ])),
            (LITEPARSE_INFO_KIND_OTHER, "[1 /A 4 0 R]".to_owned())
        );
        assert_eq!(
            kind(Object::Null),
            (LITEPARSE_INFO_KIND_OTHER, "null".to_owned())
        );
    }
}
