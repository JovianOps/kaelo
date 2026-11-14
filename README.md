# kaelo

A small, fast, memory-safe language you can embed in a game or a tool.

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

Python is quick to write and slow to run. Games and tools need something that keeps
the readable surface but doesn't stall: no null, no manual memory, no garbage
collector pauses, and a predictable per-frame cost.

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

## Embedding

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

| Workload | kaelo 0.1 | CPython 3.14 | ratio |
|---|---|---|---|
| 10M-iteration integer loop | 1514 ms | 567 ms | 2.7x slower |
| 1000x1000 nested loop | 142 ms | 67 ms | 2.1x slower |
| 20k string concatenations | 40 ms | 2.3 ms | 17x slower |
| recursive fib(25) | 24.6 ms | 14.2 ms | 1.7x slower |
| build + sum a 200k array | 63.5 ms | 22.8 ms | 2.8x slower |

**kaelo 0.1 is not faster than CPython yet.** It is a switch-dispatch bytecode VM
with a 24-byte tagged value; CPython 3.14 has inline caches and adaptive
specialisation, and beats it on every workload above. The path to parity is
superinstructions and an unboxed value stack, then a JIT — none of which are in
0.1. The language surface and the memory-safety rules are done; the speed is not.

## Status

0.1. Experimental. Language and VM are stable enough to build on; the standard
library is deliberately small (print, len, push, pop, str, int, float, math, seeded
random, clock). Nested `fn` declarations are not supported.

## License

MIT