//! Plain C ABI surface for consumers without UniFFI tooling (Windows
//! P/Invoke from C#, C++ via the generated header, anything else with
//! `extern "C"` interop).
//!
//! Results are returned as heap-allocated, null-terminated UTF-8 JSON
//! strings matching `IdentifyResult` (see SOFTWARE_DESIGN.md §5). The
//! shape is identical to the CLI's `--pretty`-less output. Callers must
//! release every non-null returned pointer via `lid_string_free` — the
//! Rust allocator owns the memory and `free()`-ing it from the C side
//! would corrupt the heap.

use std::ffi::{c_char, CStr, CString};

use language_identifier as li;

/// Identifies the language of `input` (UTF-8, null-terminated).
///
/// Returns a heap-allocated null-terminated UTF-8 JSON string matching
/// `IdentifyResult`. Returns null if `input` is null, not valid UTF-8,
/// or serialization fails.
///
/// # Safety
/// `input` must either be null or point to a valid null-terminated
/// UTF-8 C string for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn lid_identify(input: *const c_char) -> *mut c_char {
    if input.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(s) = CStr::from_ptr(input).to_str() else {
        return std::ptr::null_mut();
    };
    let result = li::identify(s);
    serde_json::to_string(&result)
        .ok()
        .and_then(|j| CString::new(j).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Identifies the language of `len` UTF-8 null-terminated lines.
///
/// Same return contract as `lid_identify`. Returns null if `lines` is
/// null, any element is null, or any element is not valid UTF-8.
///
/// # Safety
/// `lines` must point to an array of at least `len` valid
/// `*const c_char` pointers, each of which is null-terminated UTF-8.
#[no_mangle]
pub unsafe extern "C" fn lid_identify_lines(
    lines: *const *const c_char,
    len: usize,
) -> *mut c_char {
    if lines.is_null() {
        return std::ptr::null_mut();
    }
    let slice = std::slice::from_raw_parts(lines, len);
    let mut owned: Vec<String> = Vec::with_capacity(len);
    for &ptr in slice {
        if ptr.is_null() {
            return std::ptr::null_mut();
        }
        match CStr::from_ptr(ptr).to_str() {
            Ok(s) => owned.push(s.to_owned()),
            Err(_) => return std::ptr::null_mut(),
        }
    }
    let result = li::identify_lines(&owned);
    serde_json::to_string(&result)
        .ok()
        .and_then(|j| CString::new(j).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Releases a string returned by `lid_identify` / `lid_identify_lines`.
/// Calling with null is a no-op.
///
/// # Safety
/// `s` must be a pointer returned by one of the `lid_*` functions in
/// this module, never freed before. Double-free is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn lid_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    drop(CString::from_raw(s));
}
