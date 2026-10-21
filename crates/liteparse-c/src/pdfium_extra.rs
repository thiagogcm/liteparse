//! PDFium entry points the `pdfium-sys` table does not list, resolved from the
//! PDFium already loaded in this process.
//!
//! `pdfium_sys::dynamic` binds a fixed set of symbols. The reads here need a
//! few more, and the table is not extended: the library file it loaded is
//! found from the address of one of its functions, opened again (the dynamic
//! linker hands back the same mapping and counts a reference), and the extra
//! symbols are resolved from that handle. A process still holds one PDFium.

use std::ffi::{CStr, c_char, c_uint, c_ulong, c_void};
use std::path::PathBuf;
use std::sync::OnceLock;

use liteparse_pdfium::pdfium_sys::{
    FPDF_ACTION, FPDF_ANNOTATION, FPDF_BOOL, FPDF_DEST, FPDF_DOCUMENT, FPDF_LINK, FPDF_PAGE,
    FPDF_SIGNATURE, FPDF_STRUCTELEMENT, dynamic,
};

use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE};

/// `FPDFPage_Get{Media,Crop,Bleed,Trim,Art}Box`: left, bottom, right, top.
pub(crate) type PageBoxFn =
    unsafe extern "C" fn(FPDF_PAGE, *mut f32, *mut f32, *mut f32, *mut f32) -> FPDF_BOOL;

/// The extra symbols, valid for as long as the process: the library handle is
/// kept in the same value and never closed.
pub(crate) struct Extra {
    _library: libloading::Library,
    pub(crate) media_box: PageBoxFn,
    pub(crate) crop_box: PageBoxFn,
    pub(crate) bleed_box: PageBoxFn,
    pub(crate) trim_box: PageBoxFn,
    pub(crate) art_box: PageBoxFn,
    /// `FPDFAnnot_GetColor(annot, type, &r, &g, &b, &a)`.
    pub(crate) annot_color: unsafe extern "C" fn(
        FPDF_ANNOTATION,
        c_uint,
        *mut c_uint,
        *mut c_uint,
        *mut c_uint,
        *mut c_uint,
    ) -> FPDF_BOOL,
    /// `FPDFLink_GetDest(document, link)`: the link's own `/Dest`.
    pub(crate) link_dest: unsafe extern "C" fn(FPDF_DOCUMENT, FPDF_LINK) -> FPDF_DEST,
    /// `FPDFAction_GetType(action)`: `PDFACTION_*`.
    pub(crate) action_type: unsafe extern "C" fn(FPDF_ACTION) -> c_ulong,
    /// `FPDF_StructElement_GetLang(element, buffer, length)`: UTF-16LE.
    pub(crate) struct_element_lang:
        unsafe extern "C" fn(FPDF_STRUCTELEMENT, *mut c_void, c_ulong) -> c_ulong,
    pub(crate) trailer_ends: unsafe extern "C" fn(FPDF_DOCUMENT, *mut c_uint, c_ulong) -> c_ulong,
    pub(crate) signature_sub_filter:
        unsafe extern "C" fn(FPDF_SIGNATURE, *mut c_char, c_ulong) -> c_ulong,
    pub(crate) signature_time:
        unsafe extern "C" fn(FPDF_SIGNATURE, *mut c_char, c_ulong) -> c_ulong,
}

/// The extra symbols of the PDFium this process loaded. PDFium must be loaded
/// first, which any open document guarantees.
pub(crate) fn extra() -> FfiResult<&'static Extra> {
    static EXTRA: OnceLock<Result<Extra, String>> = OnceLock::new();
    EXTRA
        .get_or_init(Extra::resolve)
        .as_ref()
        .map_err(|message| FfiError::new(LITEPARSE_STATUS_DEPENDENCY_UNAVAILABLE, message.clone()))
}

impl Extra {
    fn resolve() -> Result<Self, String> {
        let path = loaded_path()?;
        // SAFETY: the file is the PDFium this process already mapped, so
        // opening it again runs no initialiser.
        let library = unsafe { libloading::Library::new(&path) }
            .map_err(|error| format!("PDFium could not be opened again: {error}"))?;
        // SAFETY: each symbol is a PDFium export with the signature declared
        // in its field's type; the pointers outlive the handle's use because
        // `Extra` owns the handle.
        unsafe {
            Ok(Self {
                media_box: symbol(&library, c"FPDFPage_GetMediaBox")?,
                crop_box: symbol(&library, c"FPDFPage_GetCropBox")?,
                bleed_box: symbol(&library, c"FPDFPage_GetBleedBox")?,
                trim_box: symbol(&library, c"FPDFPage_GetTrimBox")?,
                art_box: symbol(&library, c"FPDFPage_GetArtBox")?,
                annot_color: symbol(&library, c"FPDFAnnot_GetColor")?,
                link_dest: symbol(&library, c"FPDFLink_GetDest")?,
                action_type: symbol(&library, c"FPDFAction_GetType")?,
                struct_element_lang: symbol(&library, c"FPDF_StructElement_GetLang")?,
                trailer_ends: symbol(&library, c"FPDF_GetTrailerEnds")?,
                signature_sub_filter: symbol(&library, c"FPDFSignatureObj_GetSubFilter")?,
                signature_time: symbol(&library, c"FPDFSignatureObj_GetTime")?,
                _library: library,
            })
        }
    }
}

/// # Safety
///
/// `T` must be the function pointer type of the export called `name`.
unsafe fn symbol<T: Copy>(library: &libloading::Library, name: &CStr) -> Result<T, String> {
    // SAFETY: the caller vouches for `T`.
    unsafe { library.get::<T>(name.to_bytes_with_nul()) }
        .map(|symbol| *symbol)
        .map_err(|error| {
            format!(
                "the loaded PDFium does not export {}: {error}",
                name.to_string_lossy()
            )
        })
}

/// The file the loaded PDFium was mapped from, found from where one of its
/// functions lies.
fn loaded_path() -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStringExt;

    let anchor = dynamic::pdfium().FPDF_InitLibrary as *const c_void;
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: `info` is writable and `anchor` is a function pointer.
    let found = unsafe { libc::dladdr(anchor, info.as_mut_ptr()) };
    if found == 0 {
        return Err("the file PDFium was loaded from is unknown".to_owned());
    }
    // SAFETY: `dladdr` filled `info`.
    let name = unsafe { info.assume_init() }.dli_fname;
    if name.is_null() {
        return Err("the file PDFium was loaded from is unknown".to_owned());
    }
    // SAFETY: a NUL-terminated string owned by the dynamic linker.
    let bytes = unsafe { CStr::from_ptr(name) }.to_bytes().to_vec();
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// `PDFACTION_GOTO`: a jump to a destination of this document.
pub(crate) const PDFACTION_GOTO: c_ulong = 1;
