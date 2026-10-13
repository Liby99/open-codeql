/* CallGraph.c — exercises function calls and call binding.
 *
 *   - Direct calls
 *   - Self-recursion
 *   - Mutual recursion
 *   - Function-pointer-shaped definitions (no actual indirect call resolution required)
 */

#include <stddef.h>

int square(int x) {
    return x * x;
}

int cube(int x) {
    return x * square(x);
}

/* mutual recursion */
int is_even(int n);
int is_odd(int n);

int is_even(int n) {
    if (n == 0) return 1;
    return is_odd(n - 1);
}

int is_odd(int n) {
    if (n == 0) return 0;
    return is_even(n - 1);
}

/* self-recursion */
int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

/* deep call chain */
int level3(int x) { return x + 3; }
int level2(int x) { return level3(x) + 2; }
int level1(int x) { return level2(x) + 1; }
int level0(int x) { return level1(x); }

/* fan-in: multiple callers */
int helper(int x) {
    return x * 10;
}

int caller_a(int x) { return helper(x) + 1; }
int caller_b(int x) { return helper(x) + 2; }
int caller_c(int x) { return helper(x) + 3; }

int main(void) {
    int a = square(3);
    int b = cube(3);
    int c = factorial(5);
    int d = level0(100);
    int e = is_even(10);
    int f = is_odd(7);
    int g = caller_a(1) + caller_b(2) + caller_c(3);
    return a + b + c + d + e + f + g;
}
