// A simple C program for demonstrating ocql queries.

#include <stdio.h>

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

int compute(int x) {
    int sum = add(x, 1);
    int prod = multiply(sum, 2);
    return add(prod, x);
}

int main() {
    int result = compute(10);
    printf("result = %d\n", result);
    return 0;
}
