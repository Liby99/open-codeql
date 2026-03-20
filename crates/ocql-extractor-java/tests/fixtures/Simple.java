package com.example;

import java.util.List;
import java.util.ArrayList;

/**
 * A simple class for testing extraction.
 */
public class Simple {
    private int count;
    private String name;

    public Simple(String name) {
        this.count = 0;
        this.name = name;
    }

    public int getCount() {
        return count;
    }

    public void increment() {
        count++;
    }

    public String getName() {
        return name;
    }

    public static int factorial(int n) {
        if (n <= 1) return 1;
        return n * factorial(n - 1);
    }

    public static void main(String[] args) {
        Simple s = new Simple("test");
        s.increment();
        System.out.println(s.getName() + ": " + s.getCount());

        int result = factorial(5);
        System.out.println("5! = " + result);

        List<String> items = new ArrayList<>();
        items.add("hello");
        for (String item : items) {
            System.out.println(item);
        }
    }
}
