/* C shim between kaelo's C ABI and Goose's FFI rules.
 *
 * Goose will not pass or return a reference-to-opaque or a slice across the
 * boundary, so kaelo's handle-returning API is adapted here: the handle comes
 * back through an out-parameter, and string results are copied into a caller
 * buffer. This is the whole integration surface.
 */
#ifndef KAELO_SHIM_H
#define KAELO_SHIM_H

#include "kaelo.h"

#ifdef __cplusplus
extern "C" {
#endif

/* 0 on success; the handle is written to *out */
int shim_open(Kaelo **out);

/* Goose passes a slice as (pointer, length), not a C string, so the shim
 * takes the pair and copies it into a NUL-terminated buffer. */
typedef struct { const char *ptr; long long len; } shim_slice;

int shim_run(Kaelo *k, shim_slice source);

/* Call a script function that returns an int; result in *out */
int shim_call_int(Kaelo *k, shim_slice name, long long arg, long long *out);

/* Evaluate source and copy the printed result into buf */
/* evaluate and print the result (for hosts that would rather not marshal strings) */
int shim_eval_print(Kaelo *k, shim_slice source);

/* returns the string length, or -1 on error */
int shim_eval_str(Kaelo *k, shim_slice source, char *buf, int size);

int shim_last_error(Kaelo *k, char *buf, int size);

void shim_close(Kaelo *k);

#ifdef __cplusplus
}
#endif
#endif