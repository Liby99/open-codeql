/* Literals.c — exercises literal forms and the valuetext table. */

int int_literals(void) {
    int a = 42;
    int b = 0;
    int c = 1000;
    return a + b + c;
}

int char_literals(void) {
    char a = 'x';
    char b = '\n';
    return a + b;
}

const char *string_literals(void) {
    const char *s = "hello";
    return s;
}

int hex_octal(void) {
    int h = 0xFF;
    int o = 0777;
    return h + o;
}

int main(void) {
    int x = int_literals() + char_literals() + hex_octal();
    const char *s = string_literals();
    return x + (s != 0);
}
