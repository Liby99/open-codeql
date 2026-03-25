// TypeScript test fixture

// Interface
interface Shape {
    area(): number;
    perimeter(): number;
    name: string;
}

// Enum
enum Color {
    Red = 'red',
    Green = 'green',
    Blue = 'blue',
}

// Type alias
type StringOrNumber = string | number;
type Point = { x: number; y: number };

// Generic function
function identity<T>(value: T): T {
    return value;
}

// Class implementing interface
class Circle implements Shape {
    name: string = 'circle';
    private radius: number;

    constructor(radius: number) {
        this.radius = radius;
    }

    area(): number {
        return Math.PI * this.radius ** 2;
    }

    perimeter(): number {
        return 2 * Math.PI * this.radius;
    }

    static fromDiameter(diameter: number): Circle {
        return new Circle(diameter / 2);
    }
}

// Generic class
class Container<T> {
    private items: T[] = [];

    add(item: T): void {
        this.items.push(item);
    }

    get(index: number): T {
        return this.items[index];
    }

    get length(): number {
        return this.items.length;
    }
}

// Arrow function with type annotations
const double = (n: number): number => n * 2;

// Async function with types
async function fetchJSON<T>(url: string): Promise<T> {
    const response = await fetch(url);
    return response.json();
}

// Destructuring with types
const { x, y }: Point = { x: 1, y: 2 };

// Const assertion
const colors = ['red', 'green', 'blue'] as const;

// Conditional types usage
function processValue(value: StringOrNumber): string {
    if (typeof value === 'string') {
        return value.toUpperCase();
    }
    return value.toString();
}

// Optional chaining with types
interface User {
    name: string;
    address?: {
        street: string;
        city: string;
    };
}

function getCity(user: User): string | undefined {
    return user.address?.city;
}

// Enum usage
const favorite: Color = Color.Blue;

// Variable declarations
let mutable: number = 42;
const immutable: string = 'hello';
var legacy: boolean = true;

// Export
export { Circle, Container, Color };
export default identity;
export type { Shape, Point };
