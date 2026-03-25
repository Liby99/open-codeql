// Package main is the entry point.
package main

import (
	"fmt"
	"strings"
)

// MaxSize is the maximum allowed size.
const MaxSize = 100

// Version is the current version string.
var Version = "1.0.0"

// Point represents a 2D point.
type Point struct {
	X int
	Y int
}

// Shape is an interface for geometric shapes.
type Shape interface {
	Area() float64
	Perimeter() float64
}

// StringAlias is a type alias.
type StringAlias = string

// String returns a string representation of the point.
func (p Point) String() string {
	return fmt.Sprintf("(%d, %d)", p.X, p.Y)
}

// add returns the sum of two integers.
func add(a, b int) int {
	return a + b
}

// main is the entry point.
func main() {
	// Create a point using composite literal
	p := Point{X: 10, Y: 20}
	fmt.Println(p.String())

	// Variable declarations
	var x int = 42
	var y = add(x, 8)

	// If statement
	if y > 50 {
		fmt.Println("big")
	} else {
		fmt.Println("small")
	}

	// For loop
	sum := 0
	for i := 0; i < 10; i++ {
		sum = sum + i
	}

	// Range loop
	items := []string{"a", "b", "c"}
	for _, item := range items {
		fmt.Println(item)
	}

	// Switch statement
	switch x {
	case 1:
		fmt.Println("one")
	case 2:
		fmt.Println("two")
	default:
		fmt.Println("other")
	}

	// Short var declaration
	result := strings.Join(items, ", ")
	fmt.Println(result)

	// Go and defer
	go fmt.Println("goroutine")
	defer fmt.Println("deferred")

	// Assignment
	x = x + 1

	// Binary operations
	_ = x < y
	_ = x == y

	// Function literal
	fn := func(n int) int {
		return n * 2
	}
	fmt.Println(fn(5))

	/* Block comment */
}
