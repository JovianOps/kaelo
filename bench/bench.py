"""CPython side of the kaelo benchmark suite.

Run in-process and take the best of N so the numbers are not polluted by
interpreter start-up. Mirrors examples/*.klo line for line.
"""
import sys
import time


def best_of(fn, n):
    best = float("inf")
    for _ in range(n):
        t = time.perf_counter()
        fn()
        best = min(best, time.perf_counter() - t)
    return best * 1000.0


def loop():
    s = 0
    i = 0
    while i < 10_000_000:
        s = s + i
        i = i + 1
    return s


def grid():
    total = 0
    y = 0
    while y < 1000:
        x = 0
        while x < 1000:
            total = total + x * y
            x = x + 1
        y = y + 1
    return total


def strcat():
    # `s = s + "ab"` form: kaelo allocates each time, CPython reuses the buffer
    s = ""
    i = 0
    while i < 20_000:
        s = s + "ab"
        i = i + 1
    return len(s)


def strcat2():
    # `s += "ab"` form: kaelo grows the buffer in place
    s = ""
    i = 0
    while i < 20_000:
        s += "ab"
        i = i + 1
    return len(s)


def fib25():
    def fib(n):
        if n < 2:
            return n
        return fib(n - 1) + fib(n - 2)
    return fib(25)


def arrays():
    a = []
    i = 0
    while i < 200_000:
        a.append(i)
        i = i + 1
    s = 0
    for x in a:
        s = s + x
    return s


CASES = [("loop", loop, 1), ("grid", grid, 3), ("strcat", strcat, 3),
         ("strcat2", strcat2, 3), ("fib", fib25, 3), ("sum", arrays, 3)]

if __name__ == "__main__":
    for name, fn, n in CASES:
        print(f"{name:<10} {best_of(fn, n):8.1f} ms")
