// Test file for indirect (function-pointer) call resolution.
//
// Pattern 1 – typedef'd function pointer, single target:
//   typedef int (*IntOp)(int);
//   IntOp fp = inc;
//   fp(5);                       → indirect call to inc
//
// Pattern 2 – reassignment, multiple targets:
//   IntOp fp = add1;
//   fp(1);                       → indirect call to add1
//   fp = mul2;
//   fp(2);                       → indirect call to mul2  (fp points-to {add1, mul2})
//
// Pattern 3 – raw function pointer syntax (no typedef):
//   int (*raw)(int) = inc;
//   raw(7);                      → indirect call to inc
//
// Pattern 4 – address-of syntax:
//   IntOp fp = &inc;
//   fp(9);                       → indirect call to inc
//
// Pattern 5 – callback parameter (inter-procedural):
//   void apply(IntOp f, int x) { f(x); }
//   apply(inc, 5);               → direct call to apply
//                                  within apply, f(x) → indirect call (needs param tracking)
//
// Pattern 6 – conditional assignment:
//   IntOp fp = flag ? inc : mul2;
//   fp(3);                       → indirect call to {inc, mul2}
//
// Known direct call edges:
//   single_target   → inc (indirect via fp)
//   multi_target    → add1, mul2 (indirect via fp)
//   raw_pointer     → inc (indirect via raw)
//   addr_of         → inc (indirect via fp)
//   use_callback    → apply (direct), inc (indirect within apply via f)
//   conditional_fp  → inc, mul2 (indirect via fp)
//   chained_calls   → inc, mul2 (indirect via fp, gp)
//   no_indirect     → inc (direct)

// ---- Target functions ----

int inc(int x) {
    return x + 1;
}

int add1(int x) {
    return x + 1;
}

int mul2(int x) {
    return x * 2;
}

int negate(int x) {
    return -x;
}

// ---- Typedef ----

typedef int (*IntOp)(int);

// ---- Pattern 1: single target via typedef ----

int single_target() {
    IntOp fp = inc;
    return fp(5);
}

// ---- Pattern 2: reassignment, multiple targets ----

int multi_target(int flag) {
    IntOp fp = add1;
    int a = fp(1);
    fp = mul2;
    int b = fp(2);
    return a + b;
}

// ---- Pattern 3: raw function pointer (no typedef) ----

int raw_pointer() {
    int (*raw)(int) = inc;
    return raw(7);
}

// ---- Pattern 4: address-of syntax ----

int addr_of() {
    IntOp fp = &inc;
    return fp(9);
}

// ---- Pattern 5: callback parameter ----

int apply(IntOp f, int x) {
    return f(x);
}

int use_callback() {
    return apply(inc, 5);
}

// ---- Pattern 6: conditional function pointer ----

int conditional_fp(int flag) {
    IntOp fp;
    if (flag) {
        fp = inc;
    } else {
        fp = mul2;
    }
    return fp(3);
}

// ---- Pattern 7: chained indirect calls ----

int chained_calls() {
    IntOp fp = inc;
    IntOp gp = mul2;
    int a = fp(1);
    int b = gp(a);
    return b;
}

// ---- Pattern 8: no indirect calls (baseline) ----

int no_indirect(int x) {
    return inc(x);
}
