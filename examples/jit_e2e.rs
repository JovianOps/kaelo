//! JIT milestone 2: lower a kaelo function (locals, arithmetic, while, if,
//! return) to native code and check it against the interpreter.

use kaelo::compiler::Compiler;
use kaelo::parser;
use kaelo::vm::Value;

fn compile_fn(src: &str, name: &str, n_params: usize) -> Option<kaelo::jit::JitCode> {
    let prog = parser::parse(src).ok()?;
    let mut c = Compiler::new();
    let compiled = c.compile(&prog).ok()?;
    // find the function's chunk among the constants
    for f in &compiled.fns {
        if f.name == name {
            return kaelo::jit::compile(&f.chunk, n_params, &std::collections::HashSet::new());
        }
    }
    None
}

fn main() {
    // fn sum_to(n) { var s = 0; var i = 0; while i < n { s = s + i; i = i + 1; } return s; }
    let src = r#"
fn sum_to(n) {
    var s = 0;
    var i = 0;
    while i < n {
        s = s + i;
        i = i + 1;
    }
    return s;
}
"#;
    let jit = match compile_fn(src, "sum_to", 1) {
        Some(j) => j,
        None => {
            println!("JIT compile returned None (fell back to interpreter)");
            return;
        }
    };
    println!("sum_to compiled: {} bytes of machine code", jit.len);

    let mut ok = true;
    for n in [0i64, 1, 2, 10, 100, 1000] {
        let got = unsafe { jit.call(&[n], std::ptr::null()) };
        let want = (0..n).sum::<i64>();
        if got != want {
            println!("MISMATCH n={} jit={} want={}", n, got, want);
            ok = false;
        }
    }
    println!("correctness: {}", if ok { "OK" } else { "FAILED" });

    // timing: 200k iterations of sum_to(1000) through the JIT
    let iters = 20_000u64;
    let t = std::time::Instant::now();
    let mut acc = 0i64;
    for _ in 0..iters {
        acc = unsafe { jit.call(&[1000], std::ptr::null()) };
    }
    let d = t.elapsed().as_secs_f64();
    println!(
        "{} jit calls of sum_to(1000) in {:.1} ms => {:.0} ns/call (acc {})",
        iters,
        d * 1000.0,
        d * 1e9 / iters as f64,
        acc
    );
}