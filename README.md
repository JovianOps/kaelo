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

Not to beat CPython. Measured below, CPython is faster than kaelo on most workloads,
and the table says so.

kaelo exists for the things CPython cannot give an embedded host: no null, no manual
memory, no garbage collector (so no frame-time spikes), a small deterministic
runtime you can drop into a game or a daemon, and a C ABI for hosts that are not
Python. On straight-line integer code it is close to CPython; on recursion and
array building it is clearly behind.

## What you get

- **Bytecode VM in Rust.** No dependencies, no runtime, no GC. Deterministic frees.
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
kaelo run <file.klo>       run a program
kaelo eval <code>          run code directly
kaelo bench <file.klo>     time a program (best of 3)
kaelo --version
```

## Benchmarks

Measured in process, best of 3, Rust 1.99 release vs CPython 3.14. See
`BENCHMARKS.md` for the harness.

| Workload | kaelo | CPython 3.14 | ratio |
|---|---|---|---|
| recursive fib(25) | **1.2 ms** | 15.0 ms | **12.5x faster** |
| 20k `s += "ab"` appends | **1.9 ms** | 2.4 ms | **1.26x faster** |
| 10M-iteration integer loop (top level) | 955 ms | 587 ms | 1.63x slower |
| 1000x1000 nested loop (top level) | 103 ms | 71 ms | 1.45x slower |
| build + sum a 200k array (top level) | 33 ms | 21 ms | 1.55x slower |

**Where kaelo stands.** Functions compile to native code and are far ahead:
recursion is 12.5x faster than CPython, and `s += "ab"` is 1.26x ahead because the
buffer grows in place. Straight-line top-level code still runs in the bytecode VM
and is 1.4-1.6x behind — the loop, grid and array benchmarks are all top-level
`while` loops over globals, which the backend does not lower yet. Compiling the
top-level chunk is the remaining work.

## Status

0.1. Experimental. Language and VM are stable enough to build on; the standard
library is deliberately small (print, len, push, pop, str, int, float, math, seeded
random, clock). Nested `fn` declarations are not supported.

## License

MIT