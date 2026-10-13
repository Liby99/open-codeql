/* SecurityPatterns.c — known vulnerable C patterns (CWE examples).
 *
 *   - gets/strcpy/sprintf — buffer overflow
 *   - malloc without NULL check
 *   - format string from user input
 *   - integer overflow on multiply
 *   - signed/unsigned mix
 */

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void unsafe_gets(void) {
    char buf[64];
    gets(buf);
    printf("%s\n", buf);
}

void unsafe_strcpy(const char *input) {
    char dest[32];
    strcpy(dest, input);
}

void unsafe_sprintf(const char *input) {
    char dest[16];
    /* CWE-134: untrusted format string (also unbounded sprintf, CWE-120). */
    sprintf(dest, input);
}

void unchecked_malloc(int n) {
    int *p = (int *)malloc((size_t)n * sizeof(int));
    p[0] = 42;
    free(p);
}

void format_string_attack(char *user_input) {
    printf(user_input);
}

int integer_overflow(int a, int b) {
    return a * b;
}

int safe_add(int a, int b) {
    if (a > 0 && b > 0 && a + b < a) {
        return -1;
    }
    return a + b;
}

int main(int argc, char **argv) {
    unsafe_gets();
    if (argc > 1) {
        unsafe_strcpy(argv[1]);
        unsafe_sprintf(argv[1]);
        format_string_attack(argv[1]);
    }
    unchecked_malloc(10);
    int n = integer_overflow(100000, 100000);
    int m = safe_add(1, 2);
    return n + m;
}
