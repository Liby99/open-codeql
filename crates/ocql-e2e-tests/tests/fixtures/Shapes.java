// Java test fixture with richer structure for oqlpack testing.

interface Drawable {
    void draw();
}

abstract class Shape implements Drawable {
    private String name;
    protected double x, y;

    public Shape(String name, double x, double y) {
        this.name = name;
        this.x = x;
        this.y = y;
    }

    public String getName() { return name; }
    public abstract double area();

    public void move(double dx, double dy) {
        this.x += dx;
        this.y += dy;
    }

    public boolean isAt(double px, double py) {
        return this.x == px && this.y == py;
    }
}

class Circle extends Shape {
    private double radius;

    public Circle(double x, double y, double radius) {
        super("Circle", x, y);
        this.radius = radius;
    }

    public double area() { return 3.14159 * radius * radius; }
    public double getRadius() { return radius; }
    public void draw() { /* draw circle */ }
}

class Rectangle extends Shape {
    private double width, height;

    public Rectangle(double x, double y, double w, double h) {
        super("Rectangle", x, y);
        this.width = w;
        this.height = h;
    }

    public double area() { return width * height; }
    public double getWidth() { return width; }
    public double getHeight() { return height; }
    public void draw() { /* draw rectangle */ }

    public static Rectangle square(double x, double y, double side) {
        return new Rectangle(x, y, side, side);
    }
}

class Canvas {
    private Shape[] shapes;
    private int count;

    public Canvas(int capacity) {
        this.shapes = new Shape[capacity];
        this.count = 0;
    }

    public void addShape(Shape s) {
        if (count < shapes.length) {
            shapes[count] = s;
            count++;
        }
    }

    public int getCount() { return count; }

    public double totalArea() {
        double total = 0;
        for (int i = 0; i < count; i++) {
            total += shapes[i].area();
        }
        return total;
    }

    public static void main(String[] args) {
        Canvas canvas = new Canvas(10);
        Circle c = new Circle(0, 0, 5);
        Rectangle r = Rectangle.square(1, 1, 3);
        canvas.addShape(c);
        canvas.addShape(r);
        c.draw();
        r.draw();
        double area = canvas.totalArea();
        System.out.println("Total area: " + area);
    }
}
