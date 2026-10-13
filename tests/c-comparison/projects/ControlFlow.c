/* ControlFlow.c — exercises every C statement form.
 *
 *   if/else, switch, while, do-while, for, return, break, continue, goto, label,
 *   compound (block), expression-statement, declaration-statement.
 */

#include <stddef.h>

int classify(int x) {
    if (x < 0) {
        return -1;
    } else if (x == 0) {
        return 0;
    } else {
        return 1;
    }
}

const char *day_type(int day) {
    switch (day) {
        case 0:
        case 6:
            return "weekend";
        case 1:
        case 2:
        case 3:
        case 4:
        case 5:
            return "weekday";
        default:
            return "invalid";
    }
}

int sum_for(int n) {
    int s = 0;
    for (int i = 0; i < n; i = i + 1) {
        s = s + i;
    }
    return s;
}

int sum_while(int n) {
    int s = 0;
    int i = 0;
    while (i < n) {
        s = s + i;
        i = i + 1;
    }
    return s;
}

int sum_do_while(int n) {
    int s = 0;
    int i = 0;
    do {
        s = s + i;
        i = i + 1;
    } while (i < n);
    return s;
}

int first_negative(int *xs, int n) {
    for (int i = 0; i < n; i = i + 1) {
        if (xs[i] < 0) {
            return i;
        }
    }
    return -1;
}

int find_in_matrix(int *m, int rows, int cols, int target) {
    for (int r = 0; r < rows; r = r + 1) {
        for (int c = 0; c < cols; c = c + 1) {
            if (m[r * cols + c] == target) {
                return r * cols + c;
            }
        }
    }
    return -1;
}

int with_continue_break(int n) {
    int s = 0;
    for (int i = 0; i < n; i = i + 1) {
        if (i == 5) {
            continue;
        }
        if (i == 10) {
            break;
        }
        s = s + i;
    }
    return s;
}

int with_goto(int n) {
    int s = 0;
    int i = 0;
loop:
    if (i >= n) {
        goto done;
    }
    s = s + i;
    i = i + 1;
    goto loop;
done:
    return s;
}

int main(void) {
    int a = classify(-3);
    int b = classify(7);
    int sf = sum_for(5);
    int sw = sum_while(5);
    int sd = sum_do_while(5);
    int wb = with_continue_break(15);
    int wg = with_goto(5);
    return a + b + sf + sw + sd + wb + wg;
}
