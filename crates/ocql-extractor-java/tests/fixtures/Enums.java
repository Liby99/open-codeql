package com.example;

public enum Enums {
    RED("red", 0xFF0000),
    GREEN("green", 0x00FF00),
    BLUE("blue", 0x0000FF);

    private final String name;
    private final int rgb;

    Enums(String name, int rgb) {
        this.name = name;
        this.rgb = rgb;
    }

    public String colorName() {
        return name;
    }

    public int getRgb() {
        return rgb;
    }

    public static Enums fromName(String name) {
        for (Enums c : values()) {
            if (c.name.equals(name)) {
                return c;
            }
        }
        throw new IllegalArgumentException("Unknown color: " + name);
    }
}
