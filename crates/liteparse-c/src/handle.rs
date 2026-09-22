use std::ptr::{self, NonNull};

use crate::status::{
    FfiError, FfiResult, LITEPARSE_STATUS_OK, LiteParseStatus, guard, suppress_panics,
};

/// Borrowed, non-NUL-terminated bytes valid while their owner lives. Absent
/// and empty are both a null pointer with zero length.
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct LiteParseByteView {
    pub ptr: *const u8,
    pub len: usize,
}

/// Empty and absent both encode as null with zero length.
pub(crate) fn bytes_view(value: &[u8]) -> LiteParseByteView {
    LiteParseByteView {
        ptr: array_ptr(value),
        len: value.len(),
    }
}

/// A UTF-8 string stored in the owning view's `pool`: bytes
/// `pool[offset .. offset + len]`, followed by a NUL byte that `len` does not
/// count. Absent and empty strings are both `len == 0`; the pool starts with
/// a NUL so `{0, 0}` also reads as an empty C string. Records hold no
/// pointers, so a view's arrays and pool can be copied out of the handle and
/// read from anywhere.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LiteParseStr {
    pub offset: u32,
    pub len: u32,
}

/// String storage behind every `LiteParseStr` a handle exposes. Never
/// modified after packing, so views may point at it.
pub(crate) struct Pool(Vec<u8>);

impl Default for Pool {
    fn default() -> Self {
        Self(vec![0])
    }
}

impl Pool {
    /// Append `value` and its NUL terminator; empty strings are not stored.
    pub(crate) fn push(&mut self, value: &str) -> LiteParseStr {
        if value.is_empty() {
            return LiteParseStr::default();
        }
        let offset = self.0.len();
        self.0.reserve(value.len() + 1);
        self.0.extend_from_slice(value.as_bytes());
        self.0.push(0);
        LiteParseStr {
            offset: u32::try_from(offset).expect("string pool exceeds u32::MAX bytes"),
            len: packed_len(value.len()),
        }
    }

    pub(crate) fn push_opt(&mut self, value: Option<&str>) -> LiteParseStr {
        value.map_or_else(LiteParseStr::default, |value| self.push(value))
    }

    pub(crate) fn ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

/// Resolve an input `LiteParseStr` against the caller's pool. `len == 0` is
/// absent; anything else must lie inside the pool and be valid UTF-8.
pub(crate) fn pool_str<'a>(
    pool: &'a [u8],
    value: LiteParseStr,
    name: &str,
) -> FfiResult<Option<&'a str>> {
    if value.len == 0 {
        return Ok(None);
    }
    let start = value.offset as usize;
    let bytes = start
        .checked_add(value.len as usize)
        .and_then(|end| pool.get(start..end))
        .ok_or_else(|| {
            FfiError::invalid_argument(format!(
                "{name} range {}+{} is outside the {}-byte string pool",
                value.offset,
                value.len,
                pool.len()
            ))
        })?;
    std::str::from_utf8(bytes)
        .map(Some)
        .map_err(|error| FfiError::invalid_argument(format!("{name} is not valid UTF-8: {error}")))
}

/// Maps an opaque C handle type to its Rust state and the borrowed view it
/// exposes.
pub(crate) trait Opaque {
    type State;
    const NAME: &'static str;
}

pub(crate) trait HasView {
    type View;
    fn view(&self) -> &Self::View;
}

/// Declare a handle state whose `view` field points into its own vectors.
/// The state owns every buffer the view references, so sharing it across
/// threads is sound.
macro_rules! view_state {
    ($state:ty => $view:ty, $field:ident) => {
        unsafe impl Send for $state {}
        unsafe impl Sync for $state {}
        impl $crate::handle::HasView for $state {
            type View = $view;
            fn view(&self) -> &$view {
                &self.$field
            }
        }
    };
}
pub(crate) use view_state;

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

/// Build a state, box it, and store the handle in `out`. `out` receives null
/// on failure and must not itself be null.
pub(crate) unsafe fn create_handle<H: Opaque>(
    out: *mut *mut H,
    build: impl FnOnce() -> FfiResult<H::State>,
) -> LiteParseStatus {
    let Some(out) = NonNull::new(out) else {
        return guard(|| -> FfiResult<()> {
            Err(FfiError::invalid_argument(format!(
                "out pointer for {} must not be null",
                H::NAME
            )))
        })
        .unwrap_err();
    };
    match guard(build) {
        Ok(state) => {
            unsafe {
                out.as_ptr()
                    .write(Box::into_raw(Box::new(state)).cast::<H>())
            };
            LITEPARSE_STATUS_OK
        }
        Err(status) => {
            unsafe { out.as_ptr().write(ptr::null_mut()) };
            status
        }
    }
}

pub(crate) unsafe fn free_handle<H: Opaque>(handle: *mut H) {
    let Some(handle) = NonNull::new(handle) else {
        return;
    };
    suppress_panics(|| unsafe { drop(Box::from_raw(handle.cast::<H::State>().as_ptr())) });
}

/// Borrow a handle's view; null for a null handle.
pub(crate) unsafe fn view_of<H: Opaque>(handle: *const H) -> *const <H::State as HasView>::View
where
    H::State: HasView,
{
    unsafe { state_ref(handle) }.map_or(ptr::null(), |state| ptr::from_ref(state.view()))
}

/// Write a value through an optional out pointer.
pub(crate) unsafe fn write_out<T>(out: *mut T, value: T) {
    if let Some(out) = NonNull::new(out) {
        unsafe { out.as_ptr().write(value) };
    }
}

/// A caller string given as pointer and length. Null is absent only with a
/// zero length.
pub(crate) unsafe fn optional_str<'a>(
    ptr: *const u8,
    len: usize,
    name: &str,
) -> FfiResult<Option<&'a str>> {
    let Some(bytes) = unsafe { as_slice(ptr, len, name) }? else {
        return Ok(None);
    };
    std::str::from_utf8(bytes)
        .map(Some)
        .map_err(|error| FfiError::invalid_argument(format!("{name} is not valid UTF-8: {error}")))
}

pub(crate) unsafe fn required_str<'a>(
    ptr: *const u8,
    len: usize,
    name: &str,
) -> FfiResult<&'a str> {
    unsafe { optional_str(ptr, len, name) }?
        .ok_or_else(|| FfiError::invalid_argument(format!("{name} must not be null")))
}

pub(crate) unsafe fn optional_view_str(
    view: LiteParseByteView,
    name: &str,
) -> FfiResult<Option<String>> {
    Ok(unsafe { optional_str(view.ptr, view.len, name) }?.map(str::to_owned))
}

pub(crate) unsafe fn required_view_str(view: LiteParseByteView, name: &str) -> FfiResult<String> {
    Ok(unsafe { required_str(view.ptr, view.len, name) }?.to_owned())
}

pub(crate) unsafe fn view_bytes<'a>(view: LiteParseByteView, name: &str) -> FfiResult<&'a [u8]> {
    Ok(unsafe { as_slice(view.ptr, view.len, name) }?.unwrap_or_default())
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

/// Pointer and length of a slice as a C array pair; null when empty.
pub(crate) fn array_ptr<T>(items: &[T]) -> *const T {
    if items.is_empty() {
        ptr::null()
    } else {
        items.as_ptr()
    }
}

/// The `offset + count` sub-slice of `all`. Zero-count ranges are always
/// valid; the label is formatted only on failure.
pub(crate) fn sub<'a, T>(
    all: &'a [T],
    offset: u32,
    count: u32,
    where_: &str,
    field: &str,
) -> FfiResult<&'a [T]> {
    if count == 0 {
        return Ok(&[]);
    }
    let start = offset as usize;
    start
        .checked_add(count as usize)
        .filter(|end| *end <= all.len())
        .map(|end| &all[start..end])
        .ok_or_else(|| {
            FfiError::invalid_argument(format!(
                "{where_}.{field} range {offset}+{count} is outside 0..{}",
                all.len()
            ))
        })
}

/// Record offsets and counts are `u32`; a result that overflows them cannot
/// be represented and is a hard error.
pub(crate) fn packed_len(value: usize) -> u32 {
    u32::try_from(value).expect("packed array exceeds u32::MAX entries")
}
