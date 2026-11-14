#!/bin/sh
K=${K:-./target/release/kaelo.exe}
for f in fib sum loop grid; do
  printf "%-6s interp " "$f"; "$K" run "examples/$f.klo" 2>&1 | tail -1
  printf "%-6s jit    " "$f"; "$K" jit "examples/$f.klo" 2>&1 | tail -1
done
