// Test: Modern C++ features — lambdas, auto, range-for, smart pointers,
//       structured bindings, constexpr, static_assert

#include <memory>
#include <vector>
#include <algorithm>
#include <functional>
#include <string>
#include <map>

// constexpr function
constexpr int factorial(int n) {
    return (n <= 1) ? 1 : n * factorial(n - 1);
}

// constexpr variable
constexpr int FACT_10 = factorial(10);

// static_assert
static_assert(factorial(5) == 120, "factorial(5) should be 120");

// Class with move semantics
class Buffer {
public:
    explicit Buffer(size_t size) : size_(size), data_(new char[size]) {}
    ~Buffer() { delete[] data_; }

    // Move constructor
    Buffer(Buffer&& other) noexcept : size_(other.size_), data_(other.data_) {
        other.size_ = 0;
        other.data_ = nullptr;
    }

    // Move assignment
    Buffer& operator=(Buffer&& other) noexcept {
        if (this != &other) {
            delete[] data_;
            size_ = other.size_;
            data_ = other.data_;
            other.size_ = 0;
            other.data_ = nullptr;
        }
        return *this;
    }

    // Delete copy
    Buffer(const Buffer&) = delete;
    Buffer& operator=(const Buffer&) = delete;

    size_t size() const { return size_; }
    char* data() { return data_; }

private:
    size_t size_;
    char* data_;
};

// Function taking std::function (type-erased callable)
int apply(std::function<int(int)> f, int x) {
    return f(x);
}

// Auto return type deduction
auto make_pair(int a, int b) {
    struct Pair { int first; int second; };
    return Pair{a, b};
}

// Trailing return type
auto divide(double a, double b) -> double {
    return a / b;
}

int main() {
    // Smart pointers
    auto ptr = std::make_unique<Buffer>(1024);
    auto shared = std::make_shared<std::string>("hello");

    // Lambda expressions
    auto square = [](int x) { return x * x; };
    auto result = square(5);

    // Lambda with capture
    int multiplier = 3;
    auto times_n = [multiplier](int x) { return x * multiplier; };

    // Lambda with mutable capture
    int counter = 0;
    auto increment = [&counter]() { return ++counter; };
    increment();
    increment();

    // Generic lambda (C++14)
    auto generic_add = [](auto a, auto b) { return a + b; };
    auto sum_int = generic_add(1, 2);
    auto sum_double = generic_add(1.5, 2.5);

    // Range-based for loop
    std::vector<int> numbers = {1, 2, 3, 4, 5};
    for (const auto& n : numbers) {
        auto sq = square(n);
    }

    // Structured bindings (C++17)
    auto [first, second] = make_pair(10, 20);

    // std::map with structured bindings
    std::map<std::string, int> ages = {{"Alice", 30}, {"Bob", 25}};
    for (const auto& [name, age] : ages) {
        // use name and age
    }

    // std::function + lambda
    int applied = apply(times_n, 7);

    // if with initializer (C++17)
    if (auto val = divide(10.0, 3.0); val > 3.0) {
        // ...
    }

    return 0;
}
