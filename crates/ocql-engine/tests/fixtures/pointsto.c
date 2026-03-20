// Test file for points-to analysis on function pointers.
//
// ---- basic_assign ----
// fp = inc;  →  fp points-to {inc}
//
// ---- multi_assign ----
// fp = inc; fp = mul2;  →  fp points-to {inc, mul2}  (flow-insensitive)
//
// ---- copy_propagation ----
// fp = inc; gp = fp;  →  fp points-to {inc}, gp points-to {inc}
//
// ---- chain_copy ----
// fp = inc; gp = fp; hp = gp;  →  all point to {inc}
//
// ---- branch_merge ----
// if (c) { fp = inc; } else { fp = mul2; }
// gp = fp;  →  fp points-to {inc, mul2}, gp points-to {inc, mul2}
//
// ---- overwrite_copy ----
// fp = inc; gp = fp; fp = mul2;
// →  fp points-to {inc, mul2}, gp points-to {inc}  (flow-insensitive: gp gets all of fp)
// Note: flow-insensitive analysis means gp points-to {inc, mul2} since fp -> {inc, mul2}
//
// ---- swap_pattern ----
// fp = inc; gp = mul2; tmp = fp; fp = gp; gp = tmp;
// →  fp points-to {inc, mul2}, gp points-to {inc, mul2}, tmp points-to {inc, mul2}
// (flow-insensitive: everything merges)
//
// ---- param_binding ----
// void run(IntOp f) { f(1); }
// void caller() { run(inc); }
// →  At the call site, arg 0 = inc.  Within run, f is called.
//    param_points_to(run, f, inc) from the call site.
//
// ---- multi_caller ----
// void run(IntOp f) { f(1); }
// void caller1() { run(inc); }
// void caller2() { run(mul2); }
// →  param_points_to(run, f, {inc, mul2})
//
// ---- addr_of_copy ----
// fp = &inc; gp = fp;  →  fp points-to {inc}, gp points-to {inc}
//
// ---- no_points_to ----
// int x = 42;  →  no function pointer, no points-to facts

typedef int (*IntOp)(int);

int inc(int x)  { return x + 1; }
int mul2(int x) { return x * 2; }
int neg(int x)  { return -x; }

// Pattern 1: basic assignment
int basic_assign() {
    IntOp fp = inc;
    return fp(5);
}

// Pattern 2: multiple assignments (flow-insensitive → both targets)
int multi_assign() {
    IntOp fp = inc;
    int a = fp(1);
    fp = mul2;
    int b = fp(2);
    return a + b;
}

// Pattern 3: copy propagation
int copy_propagation() {
    IntOp fp = inc;
    IntOp gp = fp;
    return gp(3);
}

// Pattern 4: chain of copies
int chain_copy() {
    IntOp fp = inc;
    IntOp gp = fp;
    IntOp hp = gp;
    return hp(4);
}

// Pattern 5: branch merge
int branch_merge(int c) {
    IntOp fp;
    if (c) {
        fp = inc;
    } else {
        fp = mul2;
    }
    IntOp gp = fp;
    return gp(5);
}

// Pattern 6: overwrite after copy (flow-insensitive)
int overwrite_copy() {
    IntOp fp = inc;
    IntOp gp = fp;
    fp = mul2;
    return gp(6) + fp(7);
}

// Pattern 7: swap pattern
int swap_pattern() {
    IntOp fp = inc;
    IntOp gp = mul2;
    IntOp tmp = fp;
    fp = gp;
    gp = tmp;
    return fp(8) + gp(9);
}

// Pattern 8: callback parameter binding (single caller)
int run(IntOp f, int x) {
    return f(x);
}

int param_binding() {
    return run(inc, 10);
}

// Pattern 9: multiple callers → parameter gets union of targets
int multi_caller1() {
    return run(inc, 11);
}

int multi_caller2() {
    return run(mul2, 12);
}

// Pattern 10: address-of with copy
int addr_of_copy() {
    IntOp fp = &inc;
    IntOp gp = fp;
    return gp(13);
}

// Pattern 11: no function pointers (baseline)
int no_points_to(int x) {
    int a = x + 1;
    return a;
}
