// Test: C++ templates — class templates, function templates, specialization

#include <vector>
#include <string>

template<typename T>
class Stack {
public:
    void push(const T& value) { data_.push_back(value); }
    T pop() {
        T val = data_.back();
        data_.pop_back();
        return val;
    }
    bool empty() const { return data_.empty(); }
    size_t size() const { return data_.size(); }
private:
    std::vector<T> data_;
};

template<typename T, typename U>
class Pair {
public:
    Pair(T first, U second) : first_(first), second_(second) {}
    T first() const { return first_; }
    U second() const { return second_; }
private:
    T first_;
    U second_;
};

// Function template
template<typename T>
T max_of(T a, T b) {
    return (a > b) ? a : b;
}

// Template specialization
template<>
const char* max_of<const char*>(const char* a, const char* b) {
    return (strcmp(a, b) > 0) ? a : b;
}

// Variadic template
template<typename... Args>
void print_all(Args... args) {
    // fold expression (C++17)
}

// Non-type template parameter
template<int N>
struct FixedArray {
    int data[N];
    int size() const { return N; }
};

void test_templates() {
    Stack<int> s;
    s.push(42);

    Pair<int, std::string> p(1, "hello");

    int m = max_of(3, 5);

    FixedArray<10> arr;
}
