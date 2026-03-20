// Test: Header-only library pattern — include guard, inline functions,
//       forward declarations

#ifndef HEADER_ONLY_H
#define HEADER_ONLY_H

#include <cstddef>

// Forward declarations
struct Node;
class Tree;

// Constants
static const int MAX_CHILDREN = 16;
static const double EPSILON = 1e-9;

// Inline utility functions
inline int clamp(int value, int lo, int hi) {
    if (value < lo) return lo;
    if (value > hi) return hi;
    return value;
}

inline double lerp(double a, double b, double t) {
    return a + t * (b - a);
}

// Template utility
template<typename T>
inline T min_val(T a, T b) {
    return (a < b) ? a : b;
}

template<typename T>
inline T max_val(T a, T b) {
    return (a > b) ? a : b;
}

// Struct definitions
struct Point2D {
    double x;
    double y;

    double distance_to(const Point2D& other) const {
        double dx = x - other.x;
        double dy = y - other.y;
        return dx * dx + dy * dy; // squared distance
    }
};

struct AABB {
    Point2D min;
    Point2D max;

    bool contains(const Point2D& p) const {
        return p.x >= min.x && p.x <= max.x &&
               p.y >= min.y && p.y <= max.y;
    }

    bool intersects(const AABB& other) const {
        return min.x <= other.max.x && max.x >= other.min.x &&
               min.y <= other.max.y && max.y >= other.min.y;
    }

    double area() const {
        return (max.x - min.x) * (max.y - min.y);
    }
};

// Type alias
using Rect = AABB;

#endif // HEADER_ONLY_H
