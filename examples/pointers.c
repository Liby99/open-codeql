// Example for points-to analysis.

int global_x = 42;
int global_y = 99;

void basic() {
    int a = 1;
    int b = 2;
    int *p = &a;
    int *q = p;     // q also points to a
    int *r = &b;
    r = &a;         // r now also points to a
}

void swap_ptrs() {
    int x = 10;
    int y = 20;
    int *px = &x;
    int *py = &y;
    int *tmp = px;
    px = py;
    py = tmp;
}

int main() {
    basic();
    swap_ptrs();
    return 0;
}
