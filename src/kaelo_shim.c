#include "kaelo_shim.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int copy_slice(shim_slice s, char *buf, int size) {
    if (!s.ptr || s.len < 0) return -1;
    long long n = s.len;
    if (n > size - 1) n = size - 1;
    memcpy(buf, s.ptr, (size_t)n);
    buf[n] = 0;
    return 0;
}

int shim_open(Kaelo **out) {
    Kaelo *k = kaelo_open();
    if (!k) return -1;
    *out = k;
    return 0;
}

int shim_run(Kaelo *k, shim_slice source) {
    char buf[65536];
    if (copy_slice(source, buf, (int)sizeof buf) != 0) return -1;
    return kaelo_run(k, buf);
}

int shim_call_int(Kaelo *k, shim_slice name, long long arg, long long *out) {
    char nbuf[256];
    if (copy_slice(name, nbuf, (int)sizeof nbuf) != 0) return -1;
    if (kaelo_call_int(k, nbuf, arg) != 0) return -1;
    *out = arg;
    return 0;
}

int shim_eval_print(Kaelo *k, shim_slice source) {
    char src[65536];
    if (copy_slice(source, src, (int)sizeof src) != 0) return -1;
    char *s = kaelo_eval(k, src);
    if (!s) return -1;
    printf("%s\n", s);
    kaelo_free_string(s);
    return 0;
}

int shim_eval_str(Kaelo *k, shim_slice source, char *buf, int size) {
    char src[65536];
    if (copy_slice(source, src, (int)sizeof src) != 0) return -1;
    char *s = kaelo_eval(k, src);
    if (!s) return -1;
    size_t n = strlen(s);
    if (n > (size_t)size - 1) n = (size_t)size - 1;
    memcpy(buf, s, n);
    buf[n] = 0;
    kaelo_free_string(s);
    return (int)n;
}

int shim_last_error(Kaelo *k, char *buf, int size) {
    const char *e = kaelo_last_error(k);
    if (!e) {
        buf[0] = 0;
        return 0;
    }
    strncpy(buf, e, (size_t)size - 1);
    buf[size - 1] = 0;
    return 0;
}

void shim_close(Kaelo *k) {
    kaelo_close(k);
}