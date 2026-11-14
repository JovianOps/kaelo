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

/* 0 on success, -1 on script error (kaelo_last_error has the message) */
int shim_run(Kaelo *k, const char *source);

/* Call a script function that returns an int; result in *out */
int shim_call_int(Kaelo *k, const char *name, long long arg, long long *out);

/* Evaluate source and copy the printed result into buf */
int shim_eval_str(Kaelo *k, const char *source, char *buf, int size);

void shim_close(Kaelo *k);

#ifdef __cplusplus
}
#endif
#endif