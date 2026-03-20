// Test: edge cases — empty file, forward decls, macros, weird syntax

// Forward declarations
class ForwardDeclared;
struct ForwardStruct;

// Empty class
class Empty {};

// Class with only constructors/destructor
class OnlyCtors {
public:
    OnlyCtors() = default;
    OnlyCtors(int x) : x_(x) {}
    OnlyCtors(const OnlyCtors&) = default;
    OnlyCtors(OnlyCtors&&) = default;
    ~OnlyCtors() = default;
private:
    int x_ = 0;
};

// Multiple variables on one line
int a = 1, b = 2, c = 3;

// Deeply nested namespace (C++17 style parsed as nested)
namespace a { namespace b { namespace c {
    int deep_var = 42;
    void deep_func() {}
}}}

// Void function with no params
void do_nothing() {}

// Function returning pointer
int* get_null() { return nullptr; }

// Function returning reference
int& get_ref() {
    static int x = 42;
    return x;
}

// Function with lots of parameters
void many_params(int a, int b, int c, int d, int e, int f, int g, int h) {
    // does nothing
}

// Const global
const int MAGIC_NUMBER = 0xDEADBEEF;

// Static global
static int static_counter = 0;

// Struct with methods
struct MathUtils {
    static int gcd(int a, int b) {
        while (b != 0) {
            int t = b;
            b = a % b;
            a = t;
        }
        return a;
    }

    static int lcm(int a, int b) {
        return a / gcd(a, b) * b;
    }
};

// Enum inside struct
struct Widget {
    enum State { ACTIVE, INACTIVE, HIDDEN };
    State state;
    int id;
};

// Typedef of function pointer
typedef void (*callback_t)(int, void*);

// Using alias for function pointer
using Callback = void(*)(int, void*);

// Function taking function pointer
void register_callback(callback_t cb, void* user_data) {
    cb(0, user_data);
}

int main() {
    Empty e;
    OnlyCtors oc(42);
    do_nothing();
    int* p = get_null();
    int& r = get_ref();
    many_params(1, 2, 3, 4, 5, 6, 7, 8);
    int g = MathUtils::gcd(12, 8);

    Widget w = {Widget::ACTIVE, 1};

    register_callback([](int x, void* data) {}, nullptr);

    return 0;
}
