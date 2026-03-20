// Test file for local block-wise control flow analysis.
//
// In `linear`: just a sequence of statements — each flows to the next.
//   { decl a; decl b; return a+b; }
//
// In `if_branch`: if/else creates two paths.
//   { if (x) { a=1; } else { a=2; } return a; }
//   Block: entry -> if_stmt -> (then_block | else_block) -> return
//
// In `loop_cfg`: while loop creates a back edge.
//   { i=0; while(i<10) { i++; } return i; }
//   Block: entry -> init -> while -> (body -> while) | exit
//
// In `nested`: nested if inside a while.
//   { while(1) { if(x) break; y++; } return y; }

int linear(int x) {
    int a = x + 1;
    int b = a * 2;
    return a + b;
}

int if_branch(int x) {
    int a;
    if (x > 0) {
        a = 1;
    } else {
        a = 2;
    }
    return a;
}

int loop_cfg(int n) {
    int i = 0;
    while (i < n) {
        i = i + 1;
    }
    return i;
}

int nested(int x) {
    int y = 0;
    while (1) {
        if (x > 10) {
            break;
        }
        y = y + 1;
        x = x + 1;
    }
    return y;
}
