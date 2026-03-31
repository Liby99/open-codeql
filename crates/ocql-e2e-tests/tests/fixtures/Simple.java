// Simple Java class for end-to-end testing.
public class Simple {
    private int value;

    public Simple(int v) {
        this.value = v;
    }

    public int getValue() {
        return value;
    }

    public int add(int x) {
        return value + x;
    }

    public static int helper(int a, int b) {
        return a + b;
    }

    public static void main(String[] args) {
        Simple s = new Simple(42);
        int result = s.add(10);
        int sum = helper(result, 5);
        System.out.println(sum);
    }
}
