# kaelo benchmarks

Measured on Windows 11, Rust 1.99 (release, lto=fat), CPython 3.14.0.
Best of 3, both sides measured **in process** — no interpreter start-up in the numbers.

| Workload | kaelo 0.1 | CPython 3.14 | ratio |
|---|---|---|---|
| `loop` — 10M iterations of `s = s + i; i = i + 1` | 1490 ms | 688 ms | 2.2× slower |
| `grid` — 1000×1000 nested loop with `total = total + x * y` | 132 ms | 83 ms | 1.6× slower |
| `strcat` — 20k `s = s + "ab"` | 37 ms | 2.4 ms | 15× slower |
| `fib` — recursive fib(25) | 25.6 ms | 14.4 ms | 1.8× slower |
| `sum` — build a 200k array, then sum it | 66.2 ms | 21.9 ms | 3.0× slower |

Run them:

```bash
cargo build --release
python bench/bench.py       # CPython side, in-process
sh examples/bench_all.sh    # kaelo side
```

## Where this stands

kaelo 0.1 is **not faster than CPython 3.14**. It is 1.6–3.0x slower across a
representative mix, and far slower on repeated string concatenation (CPython reuses
the buffer when a string has one reference; kaelo always allocates).

Measured cost: about 15 ns per executed instruction on the `loop` case, against
about 8.5 ns for CPython's specialised interpreter. A switch-dispatch VM with a
24-byte tagged value on a `Vec` stack lands in that range; CPython 3.14 has inline
caches and adaptive specialisation.

## What closes the gap

In order of payoff per unit of work:

1. **Superinstructions** — fuse the hot pairs (`LoadLocal`+`Const`+`Add`,
   `LoadLocal`+`StoreLocal`, `LoadLocal`+`LoadLocal`+`Lt`+`JumpIfFalse`) into single
   ops. Usually 1.5–2x on loop-heavy code, no new machinery.
2. **Unboxed value stack** — keep `i64`/`f64` inline with a small tag instead of a
   24-byte enum. Cuts the per-op move cost roughly in half.
3. **Inline caches for globals** — `LoadGlobal`/`StoreGlobal` hash and bounds-check;
   cache the resolved slot in the instruction after the first hit.
4. **JIT (Cranelift)** — the only route to beating CPython on recursion-heavy code.
   Large dependency, out of scope for 0.1.

kaelo 0.1 is a bytecode VM, not a JIT, and the numbers say so.