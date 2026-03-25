// A simple Swift test file for the extractor

import Foundation
import UIKit

// MARK: - Protocol

protocol Drawable {
    func draw()
    var description: String { get }
}

// MARK: - Enum

enum Direction {
    case north
    case south
    case east
    case west

    var opposite: Direction {
        switch self {
        case .north: return .south
        case .south: return .north
        case .east: return .west
        case .west: return .east
        }
    }
}

// MARK: - Struct

struct Point {
    var x: Double
    var y: Double

    func distanceTo(other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }

    mutating func translate(dx: Double, dy: Double) {
        x += dx
        y += dy
    }
}

// Extension with protocol conformance
extension Point: Drawable {
    func draw() {
        print("Point at (\(x), \(y))")
    }

    var description: String {
        return "(\(x), \(y))"
    }
}

// MARK: - Class hierarchy

class Animal {
    let name: String
    var age: Int

    init(name: String, age: Int) {
        self.name = name
        self.age = age
    }

    func speak() -> String {
        return ""
    }
}

class Dog: Animal {
    let breed: String

    init(name: String, age: Int, breed: String) {
        self.breed = breed
        super.init(name: name, age: age)
    }

    override func speak() -> String {
        return "Woof!"
    }
}

// MARK: - Generics

func swap<T>(_ a: inout T, _ b: inout T) {
    let temp = a
    a = b
    b = temp
}

class Container<Element> {
    private var items: [Element] = []

    func add(_ item: Element) {
        items.append(item)
    }

    func get(at index: Int) -> Element? {
        guard index >= 0 && index < items.count else {
            return nil
        }
        return items[index]
    }

    subscript(index: Int) -> Element? {
        return get(at: index)
    }
}

// MARK: - Free functions and control flow

func greet(name: String, enthusiastic: Bool = false) -> String {
    if enthusiastic {
        return "Hello, \(name)!!!"
    } else {
        return "Hello, \(name)."
    }
}

func processItems(_ items: [Int]) {
    for item in items {
        if item > 10 {
            print("Large: \(item)")
        } else if item > 5 {
            print("Medium: \(item)")
        } else {
            print("Small: \(item)")
        }
    }

    var i = 0
    while i < items.count {
        i += 1
    }

    guard !items.isEmpty else {
        return
    }

    defer {
        print("Done processing")
    }

    do {
        let result = try riskyOperation()
        print(result)
    } catch {
        print("Error: \(error)")
    }
}

func riskyOperation() throws -> String {
    return "success"
}

// MARK: - Closures and optional chaining

@discardableResult
func performAction(action: () -> Void) -> Bool {
    action()
    return true
}

func demonstrateExpressions() {
    // Closure
    let doubled = [1, 2, 3].map { $0 * 2 }

    // Optional chaining
    let dog: Dog? = Dog(name: "Rex", age: 3, breed: "Labrador")
    let dogName = dog?.name

    // Force unwrap
    let forcedName = dog!.name

    // Ternary
    let isOld = dog?.age ?? 0 > 10 ? true : false

    // Type checking
    let animal: Animal = Dog(name: "Spot", age: 5, breed: "Poodle")
    if animal is Dog {
        let d = animal as! Dog
        print(d.breed)
    }

    // nil coalescing and array/dictionary literals
    let numbers: [Int] = [1, 2, 3]
    let dict: [String: Int] = ["one": 1, "two": 2]
    let tuple = (1, "hello", true)

    // Await expression (in async context)
    // let value = await fetchData()

    // Key path
    let namePath = \Animal.name

    // Try expression
    let _ = try? riskyOperation()
}

// MARK: - Access modifiers

public class PublicClass {
    public var publicVar: Int = 0
    private var privateVar: Int = 0
    fileprivate var fileprivateVar: Int = 0
    internal var internalVar: Int = 0

    open func openMethod() {}
}

// MARK: - Attributes

@objc class ObjCClass: NSObject {
    @objc dynamic var value: Int = 0

    @available(iOS 15.0, *)
    func newFeature() {}
}

/*
 Multi-line comment
 for testing block comment extraction
*/
