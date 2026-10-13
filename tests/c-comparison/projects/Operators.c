/* Operators.c — exercises every C operator form so we can compare the
 * expression-kind enumeration (`case @expr.kind of`) against CodeQL.
 */

int arithmetic(int a, int b) {
    int r = 0;
    r = a + b;       /* @addexpr 25 */
    r = a - b;       /* @subexpr 26 */
    r = a * b;       /* @mulexpr 27 */
    r = a / b;       /* @divexpr 28 */
    r = a % b;       /* @remexpr 29 */
    return r;
}

int bitwise(int a, int b) {
    int r = 0;
    r = a & b;       /* @andexpr 41 */
    r = a | b;       /* @orexpr  42 */
    r = a ^ b;       /* @xorexpr 43 */
    r = a << b;      /* @lshiftexpr 39 */
    r = a >> b;      /* @rshiftexpr 40 */
    r = ~a;          /* @complementexpr 15 */
    return r;
}

int comparisons(int a, int b) {
    int r = 0;
    r = a == b;      /* @eqexpr 44 */
    r = a != b;      /* @neexpr 45 */
    r = a < b;       /* @ltexpr 47 */
    r = a > b;       /* @gtexpr 46 */
    r = a <= b;      /* @leexpr 49 */
    r = a >= b;      /* @geexpr 48 */
    return r;
}

int logical(int a, int b) {
    int r = 0;
    r = a && b;      /* @andlogicalexpr 65 */
    r = a || b;      /* @orlogicalexpr 66 */
    r = !a;          /* @notexpr 16 */
    return r;
}

int compound_assign(int a) {
    int r = a;
    r += a;          /* @assignaddexpr 53 */
    r -= a;          /* @assignsubexpr 54 */
    r *= a;          /* @assignmulexpr 55 */
    r /= a;          /* @assigndivexpr 56 */
    r %= a;          /* @assignremexpr 57 */
    r <<= a;         /* @assignlshiftexpr 58 */
    r >>= a;         /* @assignrshiftexpr 59 */
    r &= a;          /* @assignandexpr 60 */
    r |= a;          /* @assignorexpr 61 */
    r ^= a;          /* @assignxorexpr 62 */
    return r;
}

int crements(int a) {
    int r = a;
    r++;             /* @postincrexpr 20 */
    r--;             /* @postdecrexpr 21 */
    ++r;             /* @preincrexpr 22 */
    --r;             /* @predecrexpr 23 */
    return r;
}

int misc_ops(int a, int *p) {
    int r = 0;
    r = a ? 1 : 2;          /* @conditionalexpr 24 */
    r = -a;                 /* @arithnegexpr 13 */
    r = +a;                 /* @unaryplusexpr 14 */
    r = (int)sizeof(int);   /* @runtime_sizeof 93 (+ @c_style_cast 214) */
    p = &a;                 /* @address_of 2 */
    r = *p;                 /* @indirect 4 */
    r = (a, a + 1);         /* @commaexpr 67 */
    int arr[3];
    r = arr[0];             /* @subscriptexpr 68 */
    return r;
}

int main(void) {
    int p = 0;
    int s = arithmetic(3, 4) + bitwise(5, 6) + comparisons(7, 8)
          + logical(1, 0) + compound_assign(2) + crements(9)
          + misc_ops(10, &p);
    return s;
}
