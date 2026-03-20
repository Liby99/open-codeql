/* A simple C file for testing extraction. */

#include <stdio.h>

typedef struct {
    int x;
    int y;
} Point;

int add(int a, int b) {
    return a + b;
}

void print_point(Point p) {
    printf("(%d, %d)\n", p.x, p.y);
}

int main(void) {
    Point p = {3, 4};
    print_point(p);
    int sum = add(p.x, p.y);
    printf("sum = %d\n", sum);
    return 0;
}
