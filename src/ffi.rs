//! C ABI for embedding kaelo in a host that is not Rust.
//!
//! The functions are deliberately small: open a context, run a script, call a
//! script function per frame, read the last error, free the context. Strings
//! returned to the caller are heap allocated by Rust and must be released with
//! `kaelo_free_string`.
//!
//! ```c
//! Kaelo *k = kaelo_open();
//! kaelo_run(k, "fn update(dt) { }");
//! kaelo_call_int(k, "update", 16);   // 60 Hz
//! kaelo_close(k);
//! ```

use crate::{Engine, Value};
use std::ffi::{c_char, c_double, CStr, CString};
use std::ptr;

/// Opaque handle. Treat it as a pointer; never dereference it from C.
pub struct Kaelo {
    engine: Engine,
    last_error: Option<CString>,
}

#[no_mangle]
pub extern "C" fn kaelo_open() -> *mut Kaelo {
    let boxed = Box::new(Kaelo { engine: Engine::new(), last_error: None });
    Box::into_raw(boxed)
}

#[no_mangle]
pub unsafe extern "C" fn kaelo_close(handle: *mut Kaelo) {
    if handle.is_null() {
        return;
    }
    drop(Box::from_raw(handle));
}

unsafe fn set_error(handle: &mut Kaelo, msg: impl Into<String>) {
    handle.last_error = CString::new(msg.into()).ok();
}

/// Last error message, or null. Valid until the next call on this handle.
#[no_mangle]
pub unsafe extern "C" fn kaelo_last_error(handle: *const Kaelo) -> *const c_char {
    if handle.is_null() {
        return ptr::null();
    }
    match &(*handle).last_error {
        Some(s) => s.as_ptr(),
        None => ptr::null(),
    }
}

/// Run a whole script. Returns 0 on success, -1 on error (see kaelo_last_error).
#[no_mangle]
pub unsafe extern "C" fn kaelo_run(handle: *mut Kaelo, source: *const c_char) -> i32 {
    if handle.is_null() || source.is_null() {
        return -1;
    }
    let text = match CStr::from_ptr(source).to_str() {
        Ok(t) => t,
        Err(_) => return -1,
    };
    let h = &mut *handle;
    match h.engine.run(text) {
        Ok(_) => {
            h.last_error = None;
            0
        }
        Err(e) => {
            set_error(h, e);
            -1
        }
    }
}

/// Call a script function taking one float (the usual `update(dt)`).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub unsafe extern "C" fn kaelo_call_float(handle: *mut Kaelo, name: *const c_char, arg: c_double) -> i32 {
    if handle.is_null() || name.is_null() {
        return -1;
    }
    let fname = match CStr::from_ptr(name).to_str() {
        Ok(t) => t,
        Err(_) => return -1,
    };
    let h = &mut *handle;
    match h.engine.call(fname, &[Value::Float(arg)]) {
        Ok(_) => {
            h.last_error = None;
            0
        }
        Err(e) => {
            set_error(h, e);
            -1
        }
    }
}

/// Call a script function taking one int (`update`, `tick`, `spawn`).
#[no_mangle]
pub unsafe extern "C" fn kaelo_call_int(handle: *mut Kaelo, name: *const c_char, arg: i64) -> i32 {
    if handle.is_null() || name.is_null() {
        return -1;
    }
    let fname = match CStr::from_ptr(name).to_str() {
        Ok(t) => t,
        Err(_) => return -1,
    };
    let h = &mut *handle;
    match h.engine.call(fname, &[Value::Int(arg)]) {
        Ok(_) => {
            h.last_error = None;
            0
        }
        Err(e) => {
            set_error(h, e);
            -1
        }
    }
}

/// Call a no-argument script function (`draw`, `tick`).
#[no_mangle]
pub unsafe extern "C" fn kaelo_call0(handle: *mut Kaelo, name: *const c_char) -> i32 {
    if handle.is_null() || name.is_null() {
        return -1;
    }
    let fname = match CStr::from_ptr(name).to_str() {
        Ok(t) => t,
        Err(_) => return -1,
    };
    let h = &mut *handle;
    match h.engine.call(fname, &[]) {
        Ok(_) => {
            h.last_error = None;
            0
        }
        Err(e) => {
            set_error(h, e);
            -1
        }
    }
}

/// Free a string returned by kaelo (e.g. from kaelo_eval_str).
#[no_mangle]
pub unsafe extern "C" fn kaelo_free_string(ptr_: *mut c_char) {
    if ptr_.is_null() {
        return;
    }
    drop(CString::from_raw(ptr_));
}

/// Evaluate source and return the result as a string the caller must free.
/// Returns null on error.
#[no_mangle]
pub unsafe extern "C" fn kaelo_eval(handle: *mut Kaelo, source: *const c_char) -> *mut c_char {
    if handle.is_null() || source.is_null() {
        return ptr::null_mut();
    }
    let text = match CStr::from_ptr(source).to_str() {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let h = &mut *handle;
    match h.engine.run(text) {
        Ok(v) => match CString::new(v.render()) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Err(e) => {
            set_error(h, e);
            ptr::null_mut()
        }
    }
}

/// Register a host callback with no arguments and no return (draw, present).
/// `f` must stay valid for the lifetime of the handle; the caller owns it.
#[no_mangle]
pub unsafe extern "C" fn kaelo_host_fn0(handle: *mut Kaelo, name: *const c_char, f: Option<extern "C" fn()>) {
    if handle.is_null() || name.is_null() {
        return;
    }
    let fname = match CStr::from_ptr(name).to_str() {
        Ok(t) => t.to_string(),
        Err(_) => return,
    };
    let f = match f {
        Some(f) => f,
        None => return,
    };
    let h = &mut *handle;
    h.engine.host_fn(&fname, Box::new(move |_, _| {
        f();
        Ok(Value::None)
    }));
}