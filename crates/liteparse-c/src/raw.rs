//! Raw PDFium handles on an input the core has already opened.
//!
//! The pdfium crate hands out no raw handle of its own documents, so a read
//! it does not expose, or does not do safely, opens its own copy of the same
//! input here, under the same PDFium lock.

use std::ffi::CString;
use std::marker::PhantomData;

use liteparse::types::PdfInput;
use liteparse_pdfium::{Library, pdfium_sys};

use crate::status::{FfiError, FfiResult, LITEPARSE_STATUS_PARSE_ERROR};

/// A raw PDFium document on an input the core has opened, for the reads its
/// pdfium crate does not expose. Holds `'lib`, the PDFium lock, for as long
/// as it is open.
pub(crate) struct RawDocument<'lib> {
    pub(crate) handle: pdfium_sys::FPDF_DOCUMENT,
    _lock: PhantomData<&'lib Library>,
}

impl<'lib> RawDocument<'lib> {
    /// Open `input` as the core's document was opened, to read its `what`.
    /// `input`'s bytes outlive the document: both borrow from the same
    /// caller.
    pub(crate) fn open(
        _lib: &'lib Library,
        input: &'lib PdfInput,
        password: Option<&str>,
        what: &str,
    ) -> FfiResult<Self> {
        let failed = || {
            FfiError::new(
                LITEPARSE_STATUS_PARSE_ERROR,
                format!("the document could not be opened again to read its {what}"),
            )
        };
        let password = password
            .map(|password| CString::new(password).map_err(|_| failed()))
            .transpose()?;
        let password = password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());
        let pdfium = pdfium_sys::dynamic::pdfium();
        // SAFETY: under the PDFium lock `_lib` holds; the path and password
        // outlive the call, and bytes outlive the document (see above).
        let handle = unsafe {
            match input {
                PdfInput::Path(path) => {
                    let path = CString::new(path.as_str()).map_err(|_| failed())?;
                    (pdfium.FPDF_LoadDocument)(path.as_ptr(), password)
                }
                PdfInput::Bytes(bytes) => {
                    let len = i32::try_from(bytes.len()).map_err(|_| failed())?;
                    (pdfium.FPDF_LoadMemDocument)(bytes.as_ptr().cast(), len, password)
                }
            }
        };
        if handle.is_null() {
            return Err(failed());
        }
        Ok(Self {
            handle,
            _lock: PhantomData,
        })
    }

    pub(crate) fn page(&self, index: i32) -> FfiResult<RawPage<'_>> {
        // SAFETY: the document is open.
        let handle = unsafe { (pdfium_sys::dynamic::pdfium().FPDF_LoadPage)(self.handle, index) };
        if handle.is_null() {
            return Err(FfiError::new(
                LITEPARSE_STATUS_PARSE_ERROR,
                format!(
                    "page {} could not be loaded to read its structure",
                    index + 1
                ),
            ));
        }
        Ok(RawPage {
            handle,
            _document: PhantomData,
        })
    }
}

impl Drop for RawDocument<'_> {
    fn drop(&mut self) {
        // SAFETY: every page borrows the document, so none outlives it.
        unsafe { (pdfium_sys::dynamic::pdfium().FPDF_CloseDocument)(self.handle) };
    }
}

/// A page of a [`RawDocument`], closed on drop.
pub(crate) struct RawPage<'doc> {
    pub(crate) handle: pdfium_sys::FPDF_PAGE,
    _document: PhantomData<&'doc ()>,
}

impl Drop for RawPage<'_> {
    fn drop(&mut self) {
        // SAFETY: loaded by `RawDocument::page`, which it borrows.
        unsafe { (pdfium_sys::dynamic::pdfium().FPDF_ClosePage)(self.handle) };
    }
}
