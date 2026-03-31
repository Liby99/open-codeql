// C file with struct patterns for class-like analysis.
//
// Struct: Point (x, y)
// Functions: make_point, distance, add_points, origin

typedef struct {
    int x;
    int y;
} Point;

Point make_point(int x, int y) {
    Point p;
    p.x = x;
    p.y = y;
    return p;
}

int distance(Point a, Point b) {
    int dx = a.x - b.x;
    int dy = a.y - b.y;
    return dx * dx + dy * dy;
}

Point add_points(Point a, Point b) {
    Point result;
    result.x = a.x + b.x;
    result.y = a.y + b.y;
    return result;
}

Point origin() {
    return make_point(0, 0);
}

int main() {
    Point p1 = make_point(3, 4);
    Point p2 = origin();
    int d = distance(p1, p2);
    Point p3 = add_points(p1, p2);
    return d;
}
