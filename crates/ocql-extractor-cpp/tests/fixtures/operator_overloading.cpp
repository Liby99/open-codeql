// Test: operator overloading, friend functions, conversion operators

#include <iostream>
#include <cmath>

class Complex {
public:
    Complex(double real = 0, double imag = 0) : real_(real), imag_(imag) {}

    // Arithmetic operators
    Complex operator+(const Complex& rhs) const {
        return Complex(real_ + rhs.real_, imag_ + rhs.imag_);
    }
    Complex operator-(const Complex& rhs) const {
        return Complex(real_ - rhs.real_, imag_ - rhs.imag_);
    }
    Complex operator*(const Complex& rhs) const {
        return Complex(
            real_ * rhs.real_ - imag_ * rhs.imag_,
            real_ * rhs.imag_ + imag_ * rhs.real_
        );
    }

    // Compound assignment
    Complex& operator+=(const Complex& rhs) {
        real_ += rhs.real_;
        imag_ += rhs.imag_;
        return *this;
    }

    // Comparison operators
    bool operator==(const Complex& rhs) const {
        return real_ == rhs.real_ && imag_ == rhs.imag_;
    }
    bool operator!=(const Complex& rhs) const {
        return !(*this == rhs);
    }

    // Unary operators
    Complex operator-() const {
        return Complex(-real_, -imag_);
    }

    // Conversion operator
    explicit operator double() const {
        return std::sqrt(real_ * real_ + imag_ * imag_);
    }

    // Subscript operator
    double operator[](int index) const {
        return (index == 0) ? real_ : imag_;
    }

    // Function call operator (functor)
    double operator()(double t) const {
        return real_ * std::cos(t) - imag_ * std::sin(t);
    }

    // Friend function for stream output
    friend std::ostream& operator<<(std::ostream& os, const Complex& c) {
        os << c.real_ << " + " << c.imag_ << "i";
        return os;
    }

    double real() const { return real_; }
    double imag() const { return imag_; }

private:
    double real_;
    double imag_;
};

// Free function operators
Complex operator*(double scalar, const Complex& c) {
    return Complex(scalar * c.real(), scalar * c.imag());
}

int main() {
    Complex a(1, 2);
    Complex b(3, 4);

    Complex c = a + b;
    Complex d = a * b;
    Complex e = -a;
    Complex f = 2.0 * a;

    bool eq = (a == b);
    double mag = static_cast<double>(a);
    double re = a[0];
    double val = a(3.14);

    std::cout << a << std::endl;

    return 0;
}
