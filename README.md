# kaelo

A small, memory-safe language you can embed in a game or a tool.

```rust
var i = 0;
var sum = 0;
while i < 1000000 {
    sum = sum + i;
    i = i + 1;
}
print(str(sum));
```

```
$ kaelo run example.klo
499999500000
```

## Why it exists

kaelo exists for the things CPython cannot give an embedded host: no null, no manual
memory, no garbage collector (so no frame-time spikes), a small deterministic
runtime you can drop into a game or a daemon, and a C ABI for hosts that are not
Python. With the opt-in JIT it also beats CPython on recursion; without it, it is
about 2x behind on most numeric code.

## What you get

- **Bytecode VM in Rust.** No runtime, no GC, deterministic frees. Cranelift is used
  only by the opt-in JIT, never by the interpreter.
- **Memory-safe by construction.** No pointers, no manual allocation, bounds-checked
  arrays and strings, checked integer arithmetic, immutable by default.
- **No null.** Absence is the `none` value, and using it where a value is required is
  an error — not a crash three frames later.
- **Embeddable.** Register host functions from Rust (`draw`, `input`, `spawn`), then
  call script functions per frame. The host owns the loop and the renderer.
- **No GC pauses.** Values are reference-counted and freed at scope exit, so frame
  times don't spike.

## Install

```bash
git clone https://github.com/JovianOps/kaelo
cd kaelo
cargo build --release
./target/release/kaelo run examples/hello.klo
```

Requires Rust 1.8x or newer. Nothing else.

## Language

```rust
// top-level bindings are globals
let name = "kaelo";       // immutable
var score = 0;            // mutable

fn add(a, b) {
    return a + b;
}

struct Vec2 {
    x,
    y,
}

var v = Vec2 { x: 3, y: 4 };
v.x = 6;                        // field assignment

var items = [1, 2, 3];          // arrays
push(items, 4);
score += 5;                     // compound append (grow in place)
for n in items {
    print(str(n));
}

while score < 10 {
    score = score + 1;
    if score == 5 {
        continue;
    }
    if score == 9 {
        break;
    }
}
```

Types: `int` (64-bit, checked), `float`, `bool`, `string`, `array`, `struct`,
`function`, `none`. Comments are `//` and `#`. No semicolons are needed after block
statements.

`for i in 0..n` iterates a range. The standard library covers strings
(`upper`, `lower`, `trim`, `chars`, `split`, `join`, `replace`, `starts_with`,
`ends_with`, `contains`), arrays (`push`, `pop`, `len`, `sort`, `reverse`, `slice`,
`contains`), and reflection (`type`).

## Memory safety rules

| Rule | What happens |
|---|---|
| Immutable by default | `let x = 1; x = 2;` is a compile error |
| No null | there is no null literal; `none` cannot be used as a value |
| Bounds checked | `a[9]` on a 3-element array is an error, not UB |
| Checked arithmetic | `9223372036854775807 + 1` is an error, not a wrap |
| No manual memory | nothing to free, nothing to leak, nothing to double-free |
| Frozen arrays | `let a = [1,2]; push(a, 3);` is rejected |
| Step limit | runaway loops stop with an error instead of hanging |

## Embedding from C or C++

`kaelo` builds as a `cdylib`/`staticlib`. `include/kaelo.h` is the whole surface:
open a handle, run a script, call script functions per frame, read the last error.
`examples/game_host.c` is a complete host with a 600-frame loop.

```c
Kaelo *k = kaelo_open();
kaelo_run(k, "var x = 0; fn update(dt) { x = x + 1; } fn draw() { }");
kaelo_host_fn0(k, "draw", on_draw);

for (int i = 0; i < 600; i++) {
    kaelo_call_float(k, "update", 1.0 / 60.0);
    kaelo_call0(k, "draw");
}
kaelo_close(k);
```

Build: `cargo build --release`, then link `target/release/kaelo.lib` (MSVC) or
`-lkaelo` (gcc/clang). The ABI is exercised by `tests/c_abi.rs` — 4 tests that call
the exported symbols directly, so the calling convention and the string ownership
rules are covered on every `cargo test`.

## Embedding from Rust

```rust
use kaelo::{Engine, Value};

let mut k = Engine::new();

// hand engine functions to the script
k.host_fn("rect", Box::new(|_, args| {
    // draw a rect using your renderer
    Ok(Value::None)
}));

k.load(include_str!("game.klo"))?;

// drive it from your loop
for _ in 0..60 {
    k.call("update", &[Value::from(1.0 / 60.0)])?;
    k.call("draw", &[])?;
}
```

See `examples/game_loop.rs` for a complete, runnable version (600 frames of script
logic in 0.5 ms).

## CLI

```
kaelo run <file.klo>       run a program (interpreter)
kaelo jit <file.klo>       run with the native-code JIT (opt-in)
kaelo eval <code>          run code directly
kaelo bench <file.klo>     time a program (best of 3)
kaelo --version
```

## Benchmarks

Measured in process, best of 3, Rust 1.99 release vs CPython 3.14. Harness in
`bench/bench.py`; full write-up in `BENCHMARKS.md`.

Two execution paths:

| Workload | interpreter | `--jit` | CPython | jit vs CPython |
|---|---|---|---|---|
| recursive fib(25) | 33 ms | **2.1 ms** | 15.0 ms | **7x faster** |
| 10M-iteration integer loop | 1189 ms | 1127 ms | 587 ms | 1.9x slower |
| 1000x1000 nested loop | 134 ms | 152 ms | 71 ms | 2.1x slower |
| build + sum a 200k array | 37 ms | 41 ms | 21 ms | 2.0x slower |

The interpreter is the default because it implements every guarantee. The JIT is
an opt-in accelerator (`kaelo jit`) that compiles functions with integer bodies to
native code; it does not implement checked overflow or the step limit, so it is
only offered explicitly. It is strongest on recursion, which is what it is for.

**Where kaelo stands.** Two paths, and the table above is the whole story: the
interpreter is complete and carries every guarantee; the JIT is an opt-in
accelerator that is 7x ahead of CPython on recursion and no faster elsewhere.
The remaining work for v0.2 is the top-level chunk (an `__main` wrapper with
globals through a pointer, measured at 82x on a loop) and an unboxed value stack.

## Status

0.1. The language, memory-safety rules, embedding API, C ABI and CLI are complete
and covered by 23 tests. The standard library covers strings, arrays, math,
seeded random and the clock. Nested `fn` declarations
are not supported, and the JIT does not implement checked overflow or the step
limit, which is why it is opt-in.

## License

MIT