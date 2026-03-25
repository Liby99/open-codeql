// Simple JavaScript test fixture
import { readFile } from 'fs';
import path from 'path';

/* Block comment explaining the module */

// Function declaration
function add(a, b) {
    return a + b;
}

// Function with various statements
function greet(name) {
    if (name === 'world') {
        console.log('Hello, world!');
    } else {
        console.log(`Hello, ${name}!`);
    }

    for (let i = 0; i < 10; i++) {
        if (i % 2 === 0) {
            continue;
        }
        console.log(i);
    }

    for (const item of [1, 2, 3]) {
        console.log(item);
    }

    for (const key in { a: 1, b: 2 }) {
        console.log(key);
    }

    let count = 0;
    while (count < 5) {
        count++;
    }

    do {
        count--;
    } while (count > 0);

    switch (name) {
        case 'Alice':
            return 'Hi Alice!';
        case 'Bob':
            return 'Hey Bob!';
        default:
            return `Greetings, ${name}`;
    }
}

// Arrow function
const multiply = (a, b) => a * b;

// Arrow function with block body
const divide = (a, b) => {
    if (b === 0) {
        throw new Error('Division by zero');
    }
    return a / b;
};

// Class declaration
class Animal {
    constructor(name, sound) {
        this.name = name;
        this.sound = sound;
    }

    speak() {
        return `${this.name} says ${this.sound}`;
    }

    get info() {
        return `${this.name}: ${this.sound}`;
    }

    set nickname(value) {
        this._nickname = value;
    }

    static create(name, sound) {
        return new Animal(name, sound);
    }
}

// Class with inheritance
class Dog extends Animal {
    constructor(name) {
        super(name, 'Woof');
        this.tricks = [];
    }

    learn(trick) {
        this.tricks.push(trick);
    }
}

// Various expressions
const obj = { x: 1, y: 2, z: 3 };
const { x, y, ...rest } = obj;
const arr = [1, 2, 3, ...rest];
const [first, second] = arr;

// Ternary, comma, binary ops
const result = x > 0 ? x * 2 : -x;
const combined = x + y - 1;
const logical = x > 0 && y > 0 || false;
const bitwise = x & y | 0;
const nullish = x ?? 42;

// Async/await
async function fetchData(url) {
    try {
        const response = await fetch(url);
        const data = await response.json();
        return data;
    } catch (error) {
        console.error(error);
        throw error;
    } finally {
        console.log('Done');
    }
}

// Generator function
function* range(start, end) {
    for (let i = start; i < end; i++) {
        yield i;
    }
}

// Tagged template
function tag(strings, ...values) {
    return strings.join('') + values.join('');
}
const tagged = tag`hello ${x} world ${y}`;

// Destructuring in parameters
function processUser({ name, age = 25 }) {
    return `${name} is ${age}`;
}

// Labeled statement and break
outer: for (let i = 0; i < 10; i++) {
    for (let j = 0; j < 10; j++) {
        if (i + j > 5) break outer;
    }
}

// With statement (legacy)
with (Math) {
    const val = sqrt(16);
}

// Debugger
debugger;

// Update expressions
let counter = 0;
counter++;
counter--;
++counter;
--counter;

// typeof, void, delete
const t = typeof counter;
void 0;
delete obj.x;

// instanceof, in
const isAnimal = new Dog('Rex') instanceof Animal;
const hasX = 'x' in obj;

// Export
export { add, multiply };
export default Animal;
