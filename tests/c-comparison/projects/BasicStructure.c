/* BasicStructure.c — declarations: functions, structs, unions, enums, globals, typedefs.
 *
 * Designed to exercise:
 *   - functions() / fun_decls()
 *   - usertypes() (struct, union, enum)
 *   - enumconstants()
 *   - globalvariables()
 *   - namespaces() (none in C, but kept for symmetry)
 */

#include <stddef.h>

/* === enums === */
enum Color {
    RED,
    GREEN,
    BLUE
};

enum Status {
    OK = 0,
    ERROR = 1
};

/* === structs === */
struct Point {
    int x;
    int y;
};

struct Animal {
    int age;
    char *name;
};

struct Dog {
    struct Animal base;
    char *breed;
};

/* === unions === */
union Value {
    int as_int;
    double as_double;
    char *as_str;
};

/* === typedef === */
typedef struct {
    int width;
    int height;
} Rect;

/* === globals === */
int global_count = 0;
const char *app_name = "demo";
static int internal_flag = 1;

/* === forward declaration === */
int helper(int);

/* === functions === */
int helper(int n) {
    return n * 2;
}

void set_color(struct Point *p, enum Color c) {
    p->x = (int)c;
}

int point_distance_sq(struct Point a, struct Point b) {
    int dx = a.x - b.x;
    int dy = a.y - b.y;
    return dx * dx + dy * dy;
}

int main(void) {
    struct Point p = {1, 2};
    struct Point q = {4, 6};
    int d = point_distance_sq(p, q);
    set_color(&p, RED);
    return d + helper(global_count);
}
