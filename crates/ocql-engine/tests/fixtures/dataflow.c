// Test file for local dataflow analysis.
//
// We want to track how values flow through variables within a function.
//
// In `simple_flow`:
//   a = 42          -- source: literal 42
//   b = a           -- b flows from a
//   c = b + 1       -- c flows from b (and literal 1)
//   return c        -- return flows from c
//   Dataflow chain: 42 -> a -> b -> c -> return
//
// In `branching_flow`:
//   x = input
//   if (x > 0) { y = x; } else { y = 0; }
//   z = y
//   Dataflow: input -> x -> y -> z (two paths for y)
//
// In `no_flow`:
//   a = 1
//   b = 2
//   return a + b    -- a and b are independent

int simple_flow() {
    int a = 42;
    int b = a;
    int c = b + 1;
    return c;
}

int branching_flow(int input) {
    int x = input;
    int y;
    if (x > 0) {
        y = x;
    } else {
        y = 0;
    }
    int z = y;
    return z;
}

int no_flow() {
    int a = 1;
    int b = 2;
    return a + b;
}
