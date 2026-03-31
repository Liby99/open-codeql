// C file with patterns relevant to security analysis.
//
// Patterns:
//  - gets() call (dangerous function)
//  - strcpy without bounds check
//  - malloc without NULL check
//  - format string from user input

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void dangerous_gets() {
    char buf[64];
    gets(buf);           // CWE-120: buffer overflow
    printf("%s\n", buf);
}

void dangerous_strcpy(char *input) {
    char dest[32];
    strcpy(dest, input);  // CWE-120: no bounds check
}

void unchecked_malloc() {
    int *p = malloc(100);
    *p = 42;              // CWE-690: might be NULL
    free(p);
}

void format_string(char *user_input) {
    printf(user_input);   // CWE-134: format string
}

int safe_function(int x) {
    return x + 1;
}

int main() {
    dangerous_gets();
    dangerous_strcpy("hello");
    unchecked_malloc();
    format_string("test");
    return safe_function(0);
}
