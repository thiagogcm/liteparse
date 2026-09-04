use std::ptr::{self, NonNull};

use crate::status::{
    FfiError, FfiResult, LITEPARSE_STATUS_OK, LiteParseStatus, guard, suppress_panics,
};

/// Borrowed, non-NUL-terminated bytes valid while the owner lives.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct LiteParseByteView {
    pub ptr: *const u8,
    pub len: usize,
}

pub(crate) fn bytes_view(value: &[u8]) -> LiteParseByteView {
    LiteParseByteView {
        ptr: value.as_ptr(),
        len: value.len(),
    }
}

pub(crate) fn optional_str_view(value: Option<&str>) -> LiteParseByteView {
    value.map_or_else(LiteParseByteView::default, |value| {
        bytes_view(value.as_bytes())
    })
}

pub(crate) trait Opaque {
    type State;
    const NAME: &'static str;
}

macro_rules! opaque_handles {
    ($($handle:ty => $state:ty, $name:literal;)*) => {$(
        impl $crate::handle::Opaque for $handle {
            type State = $state;
            const NAME: &'static str = $name;
        }
    )*};
}
pub(crate) use opaque_handles;

fn null_handle<H: Opaque>() -> FfiError {
    FfiError::invalid_argument(format!("{} must not be null", H::NAME))
}

pub(crate) unsafe fn state_ref<'a, H: Opaque>(handle: *const H) -> FfiResult<&'a H::State> {
    NonNull::new(handle.cast_mut())
        .map(|handle| unsafe { handle.cast::<H::State>().as_ref() })
        .ok_or_else(null_handle::<H>)
}

pub(crate) unsafe fn state_mut<'a, H: Opaque>(handle: *mut H) -> FfiResult<&'a mut H::State> {
    NonNull::new(handle)
        .map(|handle| unsafe { handle.cast::<H::State>().as_mut() })
        .ok_or_else(null_handle::<H>)
}

pub(crate) fn build_handle<H: Opaque>(
    build: impl FnOnce() -> FfiResult<H::State>,
) -> (LiteParseStatus, *mut H) {
    match guard(build) {
        Ok(state) => (
            LITEPARSE_STATUS_OK,
            Box::into_raw(Box::new(state)).cast::<H>(),
        ),
        Err(status) => (status, ptr::null_mut()),
    }
}

pub(crate) unsafe fn free_handle<H: Opaque>(handle: *mut H) {
    let Some(handle) = NonNull::new(handle) else {
        return;
    };
    suppress_panics(|| unsafe { drop(Box::from_raw(handle.cast::<H::State>().as_ptr())) });
}

pub(crate) unsafe fn write_out<T: Default>(out: *mut T, value: Option<T>) {
    if let Some(out) = NonNull::new(out) {
        unsafe { out.as_ptr().write(value.unwrap_or_default()) };
    }
}

/// Returns null and sets `out_len` to zero for missing or empty slices.
pub(crate) unsafe fn slice_out<'a, T: 'a>(
    out_len: *mut usize,
    lookup: impl FnOnce() -> FfiResult<Option<&'a [T]>>,
) -> *const T {
    let slice = guard(lookup).ok().flatten().unwrap_or_default();
    unsafe { write_out(out_len, Some(slice.len())) };
    if slice.is_empty() {
        ptr::null()
    } else {
        slice.as_ptr()
    }
}

pub(crate) unsafe fn optional_view_str(
    view: LiteParseByteView,
    name: &str,
) -> FfiResult<Option<String>> {
    if view.ptr.is_null() {
        if view.len != 0 {
            return Err(FfiError::invalid_argument(format!(
                "{name} must have length zero when its pointer is null"
            )));
        }
        return Ok(None);
    }
    let bytes = unsafe { std::slice::from_raw_parts(view.ptr, view.len) };
    std::str::from_utf8(bytes)
        .map(|value| Some(value.to_owned()))
        .map_err(|error| FfiError::invalid_argument(format!("{name} is not valid UTF-8: {error}")))
}

pub(crate) unsafe fn required_view_str(view: LiteParseByteView, name: &str) -> FfiResult<String> {
    unsafe { optional_view_str(view, name) }?
        .ok_or_else(|| FfiError::invalid_argument(format!("{name} must not be null")))
}

/// Null is valid only when `len` is zero.
pub(crate) unsafe fn as_slice<'a, T>(
    items: *const T,
    len: usize,
    name: &str,
) -> FfiResult<Option<&'a [T]>> {
    if items.is_null() {
        if len != 0 {
            return Err(FfiError::invalid_argument(format!(
                "{name} must not be null when {name}_len is non-zero"
            )));
        }
        return Ok(None);
    }
    if len > (isize::MAX as usize) / size_of::<T>().max(1) {
        return Err(FfiError::invalid_argument(format!(
            "{name}_len must not exceed isize::MAX bytes"
        )));
    }
    Ok(Some(unsafe { std::slice::from_raw_parts(items, len) }))
}

pub(crate) unsafe fn copy_array<T: Copy>(
    items: *const T,
    len: usize,
    name: &str,
) -> FfiResult<Option<Vec<T>>> {
    Ok(unsafe { as_slice(items, len, name) }?.map(<[T]>::to_vec))
}
