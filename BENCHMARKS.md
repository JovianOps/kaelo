# kaelo benchmarks — measured on this machine (Windows 11, Ryzen + GTX 1650, Rust 1.99, Python 3.14)

All numbers are wall-clock, best-of-3, release build.

| Workload | kaelo | CPython 3.14 | ratio |
|---|---|---|---|
| `loop.klo` — 10M iterations of `s = s + i; i = i + 1` | 1.44 s | 2.34 s | **1.6× faster** |
| `sum.klo` — build a 200k array, then sum it | 63 ms | 62 ms | parity |
| `fib.klo` — recursive fib(25) | 23.4 ms | 13.0 ms | 1.8× slower |

Run them yourself:

```bash
cargo build --release
kaelo run examples/loop.klo
kaelo bench examples/fib.klo
```

## What this means

kaelo is a bytecode VM, not a JIT. On straight-line integer work it beats CPython
because every operation avoids Python's object boxing and dictionary lookups. On
recursion-heavy code CPython's specialised call path is still ahead.

The gap is the call path: ~95 ns per kaelo call vs ~53 ns per CPython call. Closing
it needs either a frame freelist or a JIT — neither is in 0.1.

## Embedding (game loop)

The host owns the loop and the renderer; the script owns the rules. Host functions
are registered by name and called from script like any other function.

```
600 frames of script logic in 0.5 ms  (no GC pauses, deterministic)
```

See `examples/game_loop.rs`.