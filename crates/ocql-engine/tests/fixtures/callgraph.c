// Test file for call graph resolution.
//
// Call graph:
//   main -> foo, bar
//   foo  -> bar, baz
//   bar  -> baz
//   baz  -> (nothing)
//
// Transitive from main: foo, bar, baz
// Transitive from foo:  bar, baz
// Transitive from bar:  baz

int baz(int x) {
    return x + 1;
}

int bar(int x) {
    return baz(x) + 2;
}

int foo(int x) {
    int a = bar(x);
    int b = baz(x);
    return a + b;
}

int main() {
    int r1 = foo(10);
    int r2 = bar(20);
    return r1 + r2;
}
