package com.test;

import java.util.ArrayList;
import java.util.List;

/**
 * Call graph test — method calls, virtual dispatch, constructor chains.
 * Tests callableBinding extraction parity.
 */

interface Logger {
    void log(String message);
}

class ConsoleLogger implements Logger {
    @Override
    public void log(String message) {
        System.out.println("[LOG] " + message);
    }
}

class FileLogger implements Logger {
    private String filename;

    public FileLogger(String filename) {
        this.filename = filename;
    }

    @Override
    public void log(String message) {
        // Would write to file
        System.out.println("[FILE:" + filename + "] " + message);
    }

    public String getFilename() { return filename; }
}

abstract class Processor {
    protected Logger logger;

    public Processor(Logger logger) {
        this.logger = logger;
    }

    public final void execute(String input) {
        logger.log("Processing: " + input);
        String result = process(input);
        logger.log("Result: " + result);
        onComplete(result);
    }

    protected abstract String process(String input);

    protected void onComplete(String result) {
        // Default: do nothing. Subclasses can override.
    }
}

class UpperCaseProcessor extends Processor {
    public UpperCaseProcessor(Logger logger) {
        super(logger);
    }

    @Override
    protected String process(String input) {
        return input.toUpperCase();
    }
}

class ReverseProcessor extends Processor {
    public ReverseProcessor(Logger logger) {
        super(logger);
    }

    @Override
    protected String process(String input) {
        return new StringBuilder(input).reverse().toString();
    }

    @Override
    protected void onComplete(String result) {
        logger.log("Reverse complete, length: " + result.length());
    }
}

// Chain of method calls
class Pipeline {
    private List<Processor> processors = new ArrayList<>();
    private Logger logger;

    public Pipeline(Logger logger) {
        this.logger = logger;
    }

    public Pipeline addProcessor(Processor p) {
        processors.add(p);
        return this;
    }

    public void run(String input) {
        logger.log("Pipeline start");
        for (Processor p : processors) {
            p.execute(input);
        }
        logger.log("Pipeline end");
    }

    public int getProcessorCount() {
        return processors.size();
    }
}

// Constructor chains
class Base {
    protected int id;

    public Base() {
        this(0);
    }

    public Base(int id) {
        this.id = id;
    }

    public int getId() { return id; }
}

class Derived extends Base {
    private String label;

    public Derived(String label) {
        this(0, label);
    }

    public Derived(int id, String label) {
        super(id);
        this.label = label;
    }

    public String getLabel() { return label; }
}

// Static method calls and field access
class MathHelper {
    public static final double PI = 3.14159;

    public static double circleArea(double radius) {
        return PI * radius * radius;
    }

    public static double squareArea(double side) {
        return side * side;
    }

    public static double max(double a, double b) {
        return a > b ? a : b;
    }
}

public class CallGraph {
    public static void main(String[] args) {
        // Virtual dispatch
        Logger logger = new ConsoleLogger();
        Logger fileLogger = new FileLogger("output.log");

        // Constructor chains
        Derived d = new Derived("test");

        // Method call chain
        Pipeline pipeline = new Pipeline(logger);
        pipeline.addProcessor(new UpperCaseProcessor(logger))
                .addProcessor(new ReverseProcessor(fileLogger));
        pipeline.run("hello world");

        // Static calls
        double area = MathHelper.circleArea(5.0);
        double max = MathHelper.max(area, 100.0);
        System.out.println("Max: " + max);
    }
}
