package com.test;

import java.util.List;
import java.util.ArrayList;
import java.util.Map;

/**
 * Basic Java structure test — classes, interfaces, enums, fields, methods.
 * Designed for extraction parity testing between codeql and ocodeql.
 */

// Interface with default method
interface Printable {
    void print();
    default String format() { return toString(); }
}

// Abstract class
abstract class Animal {
    private String name;
    protected int age;

    public Animal(String name, int age) {
        this.name = name;
        this.age = age;
    }

    public String getName() { return name; }
    public int getAge() { return age; }
    public abstract String speak();

    @Override
    public String toString() {
        return name + " (age " + age + ")";
    }
}

// Concrete class with inheritance and interface
class Dog extends Animal implements Printable {
    private String breed;

    public Dog(String name, int age, String breed) {
        super(name, age);
        this.breed = breed;
    }

    public String getBreed() { return breed; }

    @Override
    public String speak() { return "Woof!"; }

    @Override
    public void print() {
        System.out.println(toString() + " - " + breed);
    }
}

// Another concrete class
class Cat extends Animal implements Printable {
    private boolean indoor;

    public Cat(String name, int age, boolean indoor) {
        super(name, age);
        this.indoor = indoor;
    }

    public boolean isIndoor() { return indoor; }

    @Override
    public String speak() { return "Meow!"; }

    @Override
    public void print() {
        System.out.println(toString());
    }
}

// Enum
enum Color {
    RED, GREEN, BLUE;

    public String lower() {
        return name().toLowerCase();
    }
}

// Static utility class
final class StringUtils {
    private StringUtils() {}

    public static boolean isEmpty(String s) {
        return s == null || s.length() == 0;
    }

    public static String repeat(String s, int count) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < count; i++) {
            sb.append(s);
        }
        return sb.toString();
    }
}

// Main class
public class BasicStructure {
    private List<Animal> animals = new ArrayList<>();

    public void addAnimal(Animal a) {
        animals.add(a);
    }

    public int getCount() {
        return animals.size();
    }

    public Animal findByName(String name) {
        for (Animal a : animals) {
            if (a.getName().equals(name)) {
                return a;
            }
        }
        return null;
    }

    public static void main(String[] args) {
        BasicStructure bs = new BasicStructure();
        bs.addAnimal(new Dog("Rex", 5, "Shepherd"));
        bs.addAnimal(new Cat("Whiskers", 3, true));

        Animal found = bs.findByName("Rex");
        if (found != null) {
            System.out.println(found.speak());
        }

        Color c = Color.RED;
        System.out.println(c.lower());
    }
}
