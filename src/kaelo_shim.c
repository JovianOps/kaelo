#include "kaelo_shim.h"
#include <stdio.h>
#include <string.h>

int shim_open(Kaelo **out) {
    Kaelo *k = kaelo_open();
    if (!k) return -1;
    *out = k;
    return 0;
}

int shim_run(Kaelo *k, const char *source) {
    return kaelo_run(k, source);
}

int shim_call_int(Kaelo *k, const char *name, long long arg, long long *out) {
    if (kaelo_call_int(k, name, arg) != 0) return -1;
    *out = arg;
    return 0;
}

int shim_eval_str(Kaelo *k, const char *source, char *buf, int size) {
    char *s = kaelo_eval(k, source);
    if (!s) return -1;
    strncpy(buf, s, (size_t)size - 1);
    buf[size - 1] = 0;
    kaelo_free_string(s);
    return 0;
}

void shim_close(Kaelo *k) {
    kaelo_close(k);
}