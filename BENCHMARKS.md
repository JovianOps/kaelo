# kaelo benchmarks

Measured on Windows 11, Rust 1.99 (release, lto=fat), CPython 3.14.0.
Best of 3, both sides measured **in process** — no interpreter start-up in the numbers.

| Workload | kaelo 0.1 | CPython 3.14 | ratio |
|---|---|---|---|
| `loop` — 10M iterations of `s = s + i; i = i + 1` | 1321 ms | 628 ms | 2.1x slower |
| `grid` — 1000x1000 nested loop with `total = total + x * y` | 125 ms | 73 ms | 1.7x slower |
| `strcat2` — 20k `s += "ab"` (compound append) | **2.0 ms** | 2.4 ms | **1.2x faster** |
| `strcat` — 20k `s = s + "ab"` (allocating form) | 25.8 ms | 2.4 ms | 11x slower |
| `fib` — recursive fib(25) | 23.4 ms | 15.4 ms | 1.5x slower |
| `sum` — build a 200k array, then sum it | 62.5 ms | 22.4 ms | 2.8x slower |

Best of 3 on both sides. The `loop` number in particular moves 10% between runs on
this machine, so treat 1.5-2.8x as the real band rather than the exact digits.

Run them:

```bash
cargo build --release
python bench/bench.py       # CPython side, in-process
sh examples/bench_all.sh    # kaelo side
```

## Where this stands

kaelo 0.1 is **faster than CPython 3.14 on one workload and slower on the rest**.
It wins string building with `+=` (2.0 ms vs 2.4 ms) and is 1.5-2.8x slower on
integer loops, nested loops, recursion and array building.

`s += "ab"` is the fast path: the `+=` operator moves the slot value out before
growing it, so the buffer is uniquely owned and grows in place — 20k appends in
2.0 ms, faster than CPython. `s = s + "ab"` still allocates every iteration
(32 ms) because the stack holds a second reference to the left operand, which is
the same reason CPython falls back to allocation there.

With that fixed, the remaining gap is entirely in the VM core: integer-heavy loops
and recursion, where CPython's inline caches and adaptive specialisation win.

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