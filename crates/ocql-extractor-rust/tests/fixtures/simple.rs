// A simple Rust file for testing the extractor.

use std::collections::HashMap;
use std::fmt;
use std::io::Result as IoResult;

/// A module with nested items.
mod inner {
    pub fn helper() -> u32 {
        42
    }
}

// A simple struct with named fields.
#[derive(Debug, Clone)]
struct Point {
    x: f64,
    y: f64,
}

struct Config {
    name: String,
    value: i32,
}

// An enum with various variant kinds.
enum Color {
    Red,
    Green,
    Blue,
    Custom(u8, u8, u8),
}

enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
}

// A trait definition.
trait Describable {
    fn describe(&self) -> String;
    fn name(&self) -> &str;
}

// Inherent impl block.
impl Point {
    fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

// Trait impl block.
impl Describable for Point {
    fn describe(&self) -> String {
        format!("Point({}, {})", self.x, self.y)
    }

    fn name(&self) -> &str {
        "Point"
    }
}

// Type alias.
type PointMap = HashMap<String, Point>;

// Constants and statics.
const MAX_SIZE: usize = 1024;
static GREETING: &str = "Hello";

// A function with generics and trait bounds.
fn process<T: fmt::Display + Clone>(item: T) -> String {
    format!("{}", item)
}

// A function with various statement and expression types.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn greet(name: &str) {
    // A line comment inside a function.
    let result = format!("Hello, {}!", name);
    println!("{}", result);
}

fn main() {
    let p = Point::new(1.0, 2.0);
    let q = Point { x: 4.0, y: 6.0 };
    let d = p.distance(&q);

    /* A block comment */
    let sum = add(3, 4);

    let colors = vec![Color::Red, Color::Green, Color::Blue];

    // If/else statement
    if sum > 5 {
        println!("big: {}", sum);
    } else {
        println!("small: {}", sum);
    }

    // Match statement
    let color = Color::Red;
    match color {
        Color::Red => println!("red"),
        Color::Green => println!("green"),
        Color::Blue => println!("blue"),
        Color::Custom(r, g, b) => println!("custom: {} {} {}", r, g, b),
    }

    // For loop
    for i in 0..10 {
        let _ = i * 2;
    }

    // While loop
    let mut counter = 0;
    while counter < 5 {
        counter += 1;
    }

    // Closures
    let double = |x: i32| x * 2;
    let result = double(21);

    // Method chains and ? operator
    let desc = p.describe();

    // Tuple and array expressions
    let tuple = (1, 2, 3);
    let array = [10, 20, 30];

    // Range expression
    let range = 0..100;

    // Reference and dereference
    let r = &sum;
    let _v = *r;

    // Assignment
    let mut x = 0;
    x = 42;
    x += 1;

    // Macro invocations
    println!("done: {} {} {}", d, result, desc);
    vec![1, 2, 3];

    // Boolean literals
    let _flag = true;
    let _other = false;
}
