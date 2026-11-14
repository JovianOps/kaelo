/* Minimal C host: a game loop driving kaelo script functions.
 * Build (MSVC):  cl /I include examples\game_host.c /link target\release\kaelo.lib
 * Build (gcc):   gcc -I include examples/game_host.c -L target/release -lkaelo
 */
#include <stdio.h>
#include "kaelo.h"

static int frames = 0;

static void on_draw(void) {
    frames++;
}

int main(void) {
    Kaelo *k = kaelo_open();
    if (!k) { fprintf(stderr, "kaelo_open failed\n"); return 1; }

    const char *script =
        "var x = 0;\n"
        "var dir = 1;\n"
        "fn update(dt) {\n"
        "    x = x + dir;\n"
        "    if x > 30 { dir = 0 - 1; }\n"
        "    if x < 0 { dir = 1; }\n"
        "}\n"
        "fn draw() { draw(); }\n";

    if (kaelo_run(k, script) != 0) {
        fprintf(stderr, "script error: %s\n", kaelo_last_error(k));
        kaelo_close(k);
        return 1;
    }

    kaelo_host_fn0(k, "draw", on_draw);

    for (int i = 0; i < 600; i++) {
        if (kaelo_call_float(k, "update", 1.0 / 60.0) != 0) {
            fprintf(stderr, "update error: %s\n", kaelo_last_error(k));
            kaelo_close(k);
            return 1;
        }
        if (kaelo_call0(k, "draw") != 0) {
            fprintf(stderr, "draw error: %s\n", kaelo_last_error(k));
            kaelo_close(k);
            return 1;
        }
    }

    char *x = kaelo_eval(k, "x;");
    printf("after 600 frames: x = %s, draw() called %d times\n", x ? x : "?", frames);
    kaelo_free_string(x);

    kaelo_close(k);
    return 0;
}
