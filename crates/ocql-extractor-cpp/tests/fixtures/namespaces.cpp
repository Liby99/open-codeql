// Test: C++ namespaces — nested, anonymous, using declarations

namespace math {

double pi = 3.14159265358979;

double square(double x) {
    return x * x;
}

double cube(double x) {
    return x * x * x;
}

namespace geometry {

struct Circle {
    double radius;
    double area() const { return pi * radius * radius; }
};

struct Rectangle {
    double width;
    double height;
    double area() const { return width * height; }
};

double triangle_area(double base, double height) {
    return 0.5 * base * height;
}

} // namespace geometry

namespace linear_algebra {

struct Vec3 {
    double x, y, z;
};

double dot(const Vec3& a, const Vec3& b) {
    return a.x * b.x + a.y * b.y + a.z * b.z;
}

Vec3 cross(const Vec3& a, const Vec3& b) {
    return {
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x
    };
}

} // namespace linear_algebra

} // namespace math

// Anonymous namespace (internal linkage)
namespace {
    int internal_counter = 0;
    void increment() { internal_counter++; }
}

int main() {
    math::geometry::Circle c = {5.0};
    double a = c.area();

    math::linear_algebra::Vec3 v1 = {1, 0, 0};
    math::linear_algebra::Vec3 v2 = {0, 1, 0};
    auto v3 = math::linear_algebra::cross(v1, v2);

    increment();
    return 0;
}
