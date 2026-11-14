/* kaelo C ABI — copy to your C/C++ host.
 *
 * Build kaelo as a cdylib (cargo build --release) and link against
 * target/release/kaelo.lib (MSVC) or -lkaelo (gcc/clang).
 */
#ifndef KAELO_H
#define KAELO_H

#ifdef __cplusplus
extern "C" {
#endif

typedef struct Kaelo Kaelo;

Kaelo *kaelo_open(void);
void   kaelo_close(Kaelo *k);

/* run a whole script; 0 on success, -1 on error */
int    kaelo_run(Kaelo *k, const char *source);

/* evaluate source and return the result as a string, or NULL on error.
   release with kaelo_free_string */
char  *kaelo_eval(Kaelo *k, const char *source);
void   kaelo_free_string(char *s);

/* call a script function: no args, one int, one float */
int    kaelo_call0(Kaelo *k, const char *name);
int    kaelo_call_int(Kaelo *k, const char *name, long long arg);
int    kaelo_call_float(Kaelo *k, const char *name, double arg);

/* last error message, or NULL; valid until the next call on this handle */
const char *kaelo_last_error(const Kaelo *k);

/* register a host callback the script can call with no arguments */
void   kaelo_host_fn0(Kaelo *k, const char *name, void (*f)(void));

#ifdef __cplusplus
}
#endif
#endif /* KAELO_H */