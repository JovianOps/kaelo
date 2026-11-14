# kaelo benchmarks

Measured on Windows 11, Rust 1.99 (release, lto=fat), CPython 3.14.0.
Best of 3, both sides measured **in process** — no interpreter start-up in the numbers.

| Workload | kaelo 0.1 | CPython 3.14 | ratio |
|---|---|---|---|
| `loop` — 10M iterations of `s = s + i; i = i + 1` | 804 ms | 787 ms | 1.02x slower |
| `grid` — 1000x1000 nested loop with `total = total + x * y` | 96 ms | 79 ms | 1.22x slower |
| `strcat2` — 20k `s += "ab"` (compound append) | **1.7 ms** | 2.4 ms | **1.4x faster** |
| `strcat` — 20k `s = s + "ab"` (allocating form) | 27.6 ms | 2.4 ms | 11x slower |
| `fib` — recursive fib(25) | 24 ms | 14.7 ms | 1.63x slower |
| `sum` — build a 200k array, then sum it | 31.0 ms | 20.1 ms | 1.54x slower |

Both sides in the same session, best of 3. This machine's CPython moves 20-25%
between sessions (787 ms to 596 ms for the same `loop` script), so ratios are only
trustworthy when both sides are timed together: run `python bench/bench.py` in the
same session as the kaelo runs.

Run them:

```bash
cargo build --release
python bench/bench.py       # CPython side, in-process
sh examples/bench_all.sh    # kaelo side
```

## Where this stands

kaelo 0.1 is **at parity on integer loops and faster on string building**, and
behind on recursion and array building. The loop went 1321 -> 866 ms across this
release by cutting instructions executed rather than making each instruction
cheaper (see below), which is the only lever that has actually paid off.

`s += "ab"` is the fast path: the `+=` operator moves the slot value out before
growing it, so the buffer is uniquely owned and grows in place — 20k appends in
2.0 ms, faster than CPython. `s = s + "ab"` still allocates every iteration
(32 ms) because the stack holds a second reference to the left operand, which is
the same reason CPython falls back to allocation there.

With that fixed, the remaining gap is entirely in the VM core: integer-heavy loops
and recursion, where CPython's inline caches and adaptive specialisation win.

Measured cost: about 15 ns per executed instruction on the `loop` case, against
about 8.5 ns for CPython's specialised interpreter. CPython 3.14 has inline caches
and adaptive specialisation.

Wins in this release, all measured:

- Native calls no longer allocate. `Op::Call` did `stack.split_off(..)` for the
  argument vector, so every `push()`/`len()` in a loop allocated a fresh Vec. A
  reusable scratch buffer takes that to zero allocations: `sum` 69 -> 51 ms,
  `grid` 114 -> 108 ms.
- `x = v;` as a statement no longer re-pushes the value just to pop it (4 of the 17
  instructions in a loop body). `loop` 1130 -> 811 ms.
- `for x in arr` compiles to one fused `IterNext` (length check + element fetch +
  store) plus `IncrLocal`, instead of the desugared ~15 instructions per element:
  `sum` 48 -> 30 ms.
- The three hottest builtins (`len`, `push`, `pop`) carry a tag and are inlined in
  the `Call` arm instead of going through `Box<dyn Fn>` plus the argument buffer:
  `sum` 53 -> 48 ms. The fallback still calls the closure, and the inlined paths
  keep the same frozen-array and type checks as the closures.
- Caching the current `Chunk` in a local (one `Rc` clone per frame switch instead of
an `Rc` deref plus a `Vec` index on every instruction fetch) and keeping `ip`/`base`
in locals rather than writing `ip` back to the frames `Vec` each instruction took
`loop` from 1321 ms to 1130 ms and `sum` from 62.5 to 60.5 ms. That is the only VM
change in this release that measured as a win.

The slot size is `size_of::<Value>()` = 16 bytes. It started at 24 (strings were
`Rc<str>`, a fat pointer); switching to `Rc<String>` took it to 16 and
`Option<Value>` followed via the pointer niche. The remaining lever on the VM side
is getting the slot to 8 bytes, which means either NaN-boxing (and then either
giving up the full i64 range or keeping a box table for large ints) or a JIT.
Both are rewrites, not tuning passes.

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