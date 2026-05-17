package com.test;

/**
 * Control flow test — all statement types, exception handling, loops.
 * Tests extraction parity for statements and expressions.
 */
public class ControlFlow {
    // If/else chains
    public static String classify(int n) {
        if (n < 0) {
            return "negative";
        } else if (n == 0) {
            return "zero";
        } else if (n < 10) {
            return "small";
        } else {
            return "large";
        }
    }

    // Switch statement
    public static String dayType(int day) {
        switch (day) {
            case 1: case 7:
                return "weekend";
            case 2: case 3: case 4: case 5: case 6:
                return "weekday";
            default:
                return "unknown";
        }
    }

    // All loop types
    public static int sumFor(int n) {
        int sum = 0;
        for (int i = 1; i <= n; i++) {
            sum += i;
        }
        return sum;
    }

    public static int sumWhile(int n) {
        int sum = 0;
        int i = 1;
        while (i <= n) {
            sum += i;
            i++;
        }
        return sum;
    }

    public static int sumDoWhile(int n) {
        int sum = 0;
        int i = 1;
        do {
            sum += i;
            i++;
        } while (i <= n);
        return sum;
    }

    public static int sumEnhancedFor(int[] arr) {
        int sum = 0;
        for (int x : arr) {
            sum += x;
        }
        return sum;
    }

    // Break and continue
    public static int firstNegative(int[] arr) {
        int result = -1;
        for (int i = 0; i < arr.length; i++) {
            if (arr[i] >= 0) {
                continue;
            }
            result = arr[i];
            break;
        }
        return result;
    }

    // Labeled break
    public static boolean findInMatrix(int[][] matrix, int target) {
        boolean found = false;
        outer:
        for (int[] row : matrix) {
            for (int val : row) {
                if (val == target) {
                    found = true;
                    break outer;
                }
            }
        }
        return found;
    }

    // Try/catch/finally
    public static int safeDivide(int a, int b) {
        try {
            return a / b;
        } catch (ArithmeticException e) {
            System.err.println("Division by zero: " + e.getMessage());
            return 0;
        } finally {
            System.out.println("Division attempted");
        }
    }

    // Multiple catch
    public static Object parseValue(String s) {
        try {
            return Integer.parseInt(s);
        } catch (NumberFormatException e) {
            try {
                return Double.parseDouble(s);
            } catch (NumberFormatException e2) {
                return s;
            }
        }
    }

    // Throw
    public static void validate(int age) {
        if (age < 0) {
            throw new IllegalArgumentException("Age cannot be negative: " + age);
        }
        if (age > 150) {
            throw new IllegalArgumentException("Age too large: " + age);
        }
    }

    // Assert
    public static double sqrt(double x) {
        assert x >= 0 : "Cannot take sqrt of negative number";
        return Math.sqrt(x);
    }

    public static void main(String[] args) {
        System.out.println(classify(42));
        System.out.println(dayType(1));
        System.out.println(sumFor(10));
        System.out.println(safeDivide(10, 0));
        validate(25);
    }
}
