//! Exercises the C ABI surface directly, so the exported symbols and their
//! calling convention are covered without needing a C compiler on the host.

use kaelo::ffi::*;
use std::ffi::{CStr, CString};

#[test]
fn c_abi_run_and_eval() {
    unsafe {
        let h = kaelo_open();
        assert!(!h.is_null());

        let src = CString::new("var a = 2; var b = 3; a * b;").unwrap();
        assert_eq!(kaelo_run(h, src.as_ptr()), 0);

        let expr = CString::new("a + b;").unwrap();
        let out = kaelo_eval(h, expr.as_ptr());
        assert!(!out.is_null());
        assert_eq!(CStr::from_ptr(out).to_str().unwrap(), "5");
        kaelo_free_string(out);

        kaelo_close(h);
    }
}

#[test]
fn c_abi_call_functions() {
    unsafe {
        let h = kaelo_open();
        let script = CString::new(
            "var total = 0;\n\
             fn add(n) { total += n; }\n\
             fn tick() { total += 1; }\n\
             fn step(dt) { total += 1; }\n\
             fn read() { return total; }\n",
        )
        .unwrap();
        assert_eq!(kaelo_run(h, script.as_ptr()), 0, "script should load");

        let add = CString::new("add").unwrap();
        assert_eq!(kaelo_call_int(h, add.as_ptr(), 10), 0);
        assert_eq!(kaelo_call_int(h, add.as_ptr(), 5), 0);

        let tick = CString::new("tick").unwrap();
        assert_eq!(kaelo_call0(h, tick.as_ptr()), 0);

        let step = CString::new("step").unwrap();
        assert_eq!(kaelo_call_float(h, step.as_ptr(), 1.0 / 60.0), 0);

        let read = CString::new("read();").unwrap();
        let out = kaelo_eval(h, read.as_ptr());
        assert!(!out.is_null());
        assert_eq!(CStr::from_ptr(out).to_str().unwrap(), "17");

        kaelo_free_string(out);
        kaelo_close(h);
    }
}

#[test]
fn c_abi_reports_errors() {
    unsafe {
        let h = kaelo_open();
        let bad = CString::new("let x = 1; x = 2;").unwrap();
        assert_eq!(kaelo_run(h, bad.as_ptr()), -1);

        let err = kaelo_last_error(h);
        assert!(!err.is_null());
        let msg = CStr::from_ptr(err).to_str().unwrap();
        assert!(msg.contains("cannot assign"), "got: {}", msg);

        kaelo_close(h);
    }
}

#[test]
fn c_abi_host_callback() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn on_draw() {
        CALLS.fetch_add(1, Ordering::SeqCst);
    }

    unsafe {
        let h = kaelo_open();
        let name = CString::new("draw").unwrap();
        kaelo_host_fn0(h, name.as_ptr(), Some(on_draw));

        let script = CString::new("fn frame() { draw(); draw(); }").unwrap();
        assert_eq!(kaelo_run(h, script.as_ptr()), 0);

        let frame = CString::new("frame").unwrap();
        assert_eq!(kaelo_call0(h, frame.as_ptr()), 0);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);

        kaelo_close(h);
    }
}