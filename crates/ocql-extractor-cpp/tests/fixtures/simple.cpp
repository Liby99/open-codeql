// A simple C++ file for testing extraction.

#include <iostream>

int global_var = 42;

struct Point {
    int x;
    int y;
};

class Animal {
public:
    Animal(const char* name) : name_(name) {}
    virtual ~Animal() {}
    virtual void speak() const = 0;
    const char* name() const { return name_; }
private:
    const char* name_;
};

class Dog : public Animal {
public:
    Dog(const char* name) : Animal(name) {}
    void speak() const override {
        std::cout << name() << " says woof!" << std::endl;
    }
};

int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}

int main() {
    Dog d("Rex");
    d.speak();
    Point p = {1, 2};
    int result = factorial(5);
    return 0;
}
