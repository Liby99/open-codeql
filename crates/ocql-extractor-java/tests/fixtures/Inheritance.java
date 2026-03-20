package com.example;

import java.io.Serializable;

interface Drawable {
    void draw();
    default String description() {
        return "drawable";
    }
}

interface Resizable {
    void resize(int width, int height);
}

abstract class Shape implements Drawable, Serializable {
    protected double x, y;

    public Shape(double x, double y) {
        this.x = x;
        this.y = y;
    }

    public abstract double area();

    @Override
    public String toString() {
        return getClass().getSimpleName() + " at (" + x + ", " + y + ")";
    }
}

class Circle extends Shape implements Resizable {
    private double radius;

    public Circle(double x, double y, double radius) {
        super(x, y);
        this.radius = radius;
    }

    @Override
    public double area() {
        return Math.PI * radius * radius;
    }

    @Override
    public void draw() {
        System.out.println("Drawing circle with radius " + radius);
    }

    @Override
    public void resize(int width, int height) {
        radius = Math.min(width, height) / 2.0;
    }
}

class Rectangle extends Shape {
    private double width, height;

    public Rectangle(double x, double y, double w, double h) {
        super(x, y);
        this.width = w;
        this.height = h;
    }

    @Override
    public double area() {
        return width * height;
    }

    @Override
    public void draw() {
        System.out.println("Drawing rectangle " + width + "x" + height);
    }
}
