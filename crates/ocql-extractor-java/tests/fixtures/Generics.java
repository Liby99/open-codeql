package com.example;

import java.util.Iterator;
import java.util.NoSuchElementException;

/**
 * Generic container for testing type parameter extraction.
 */
public class Generics<T extends Comparable<T>> implements Iterable<T> {
    private Object[] data;
    private int size;

    @SuppressWarnings("unchecked")
    public Generics(int capacity) {
        data = new Object[capacity];
        size = 0;
    }

    public void add(T item) {
        if (size >= data.length) {
            throw new RuntimeException("Full");
        }
        data[size++] = item;
    }

    @SuppressWarnings("unchecked")
    public T get(int index) {
        if (index < 0 || index >= size) {
            throw new IndexOutOfBoundsException("Index: " + index);
        }
        return (T) data[index];
    }

    public int size() {
        return size;
    }

    public boolean contains(T item) {
        for (int i = 0; i < size; i++) {
            if (item.compareTo(get(i)) == 0) {
                return true;
            }
        }
        return false;
    }

    @Override
    public Iterator<T> iterator() {
        return new Iterator<T>() {
            private int pos = 0;

            @Override
            public boolean hasNext() {
                return pos < size;
            }

            @Override
            @SuppressWarnings("unchecked")
            public T next() {
                if (!hasNext()) {
                    throw new NoSuchElementException();
                }
                return (T) data[pos++];
            }
        };
    }

    public static <U extends Comparable<U>> U max(Generics<U> container) {
        if (container.size() == 0) {
            throw new IllegalArgumentException("Empty container");
        }
        U result = container.get(0);
        for (int i = 1; i < container.size(); i++) {
            U item = container.get(i);
            if (item.compareTo(result) > 0) {
                result = item;
            }
        }
        return result;
    }
}
