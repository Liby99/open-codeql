package com.example;

import java.lang.annotation.*;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.TYPE, ElementType.METHOD})
@interface MyAnnotation {
    String value() default "";
    int priority() default 0;
}

@MyAnnotation(value = "main class", priority = 1)
public class Annotations {

    @Deprecated
    private int oldField;

    private int newField;

    @MyAnnotation(value = "getter")
    public int getNewField() {
        return newField;
    }

    @Override
    public String toString() {
        return "Annotations{newField=" + newField + "}";
    }

    @SuppressWarnings("unused")
    public void unusedMethod() {
        int x = 42;
    }
}
