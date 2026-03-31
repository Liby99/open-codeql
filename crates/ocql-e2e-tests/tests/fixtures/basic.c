// Basic C file for end-to-end testing.
//
// Functions: main, helper, unused
// Params: helper(int x, int y)
// Local vars: various
// Expressions: calls, arithmetic, literals

int helper(int x, int y) {
    return x + y;
}

int unused() {
    return 0;
}

int main() {
    int a = 10;
    int b = 20;
    int c = helper(a, b);
    return c;
}
