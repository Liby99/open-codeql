// Test: C++ inheritance — single, multiple, virtual, diamond, abstract

#include <string>

class Shape {
public:
    virtual ~Shape() {}
    virtual double area() const = 0;
    virtual std::string name() const = 0;
};

class Drawable {
public:
    virtual ~Drawable() {}
    virtual void draw() const = 0;
    void set_color(int r, int g, int b) {
        r_ = r; g_ = g; b_ = b;
    }
protected:
    int r_ = 0, g_ = 0, b_ = 0;
};

class Serializable {
public:
    virtual ~Serializable() {}
    virtual std::string serialize() const = 0;
};

// Multiple inheritance
class Circle : public Shape, public Drawable, public Serializable {
public:
    Circle(double radius) : radius_(radius) {}

    double area() const override { return 3.14159 * radius_ * radius_; }
    std::string name() const override { return "Circle"; }
    void draw() const override { /* ... */ }
    std::string serialize() const override { return "circle:" + std::to_string(radius_); }

private:
    double radius_;
};

class Rectangle : public Shape, public Drawable {
public:
    Rectangle(double w, double h) : width_(w), height_(h) {}

    double area() const override { return width_ * height_; }
    std::string name() const override { return "Rectangle"; }
    void draw() const override { /* ... */ }

private:
    double width_;
    double height_;
};

// Diamond inheritance with virtual base
class Base {
public:
    int value;
    virtual ~Base() {}
};

class Left : virtual public Base {
public:
    void set_left(int v) { value = v; }
};

class Right : virtual public Base {
public:
    void set_right(int v) { value = v; }
};

class Diamond : public Left, public Right {
public:
    int get_value() const { return value; }
};

// Nested class
class Container {
public:
    class Iterator {
    public:
        Iterator(int pos) : pos_(pos) {}
        int operator*() const { return pos_; }
        Iterator& operator++() { ++pos_; return *this; }
        bool operator!=(const Iterator& other) const { return pos_ != other.pos_; }
    private:
        int pos_;
    };

    Container(int n) : n_(n) {}
    Iterator begin() const { return Iterator(0); }
    Iterator end() const { return Iterator(n_); }

private:
    int n_;
};

int main() {
    Circle c(5.0);
    Rectangle r(3.0, 4.0);

    Shape* shapes[] = {&c, &r};
    for (auto* s : shapes) {
        double a = s->area();
    }

    Diamond d;
    d.value = 42;

    Container cont(10);
    for (auto it = cont.begin(); it != cont.end(); ++it) {
        int val = *it;
    }

    return 0;
}
