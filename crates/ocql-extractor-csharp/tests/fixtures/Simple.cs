using System;
using System.Collections.Generic;
using System.Linq;

// A simple C# test file
namespace MyApp.Models
{
    /// <summary>
    /// A basic calculator class.
    /// </summary>
    [Serializable]
    public class Calculator
    {
        private int _lastResult;

        public int Add(int a, int b)
        {
            var result = a + b;
            _lastResult = result;
            return result;
        }

        public int Subtract(int a, int b)
        {
            return a - b;
        }

        public static int Multiply(int a, int b)
        {
            if (a == 0 || b == 0)
            {
                return 0;
            }
            return a * b;
        }

        public int Factorial(int n)
        {
            if (n <= 1)
                return 1;
            return n * Factorial(n - 1);
        }

        public int LastResult
        {
            get { return _lastResult; }
        }
    }

    public interface IShape
    {
        double GetArea();
        string Name { get; }
    }

    public class Circle : IShape
    {
        private readonly double _radius;

        public Circle(double radius)
        {
            _radius = radius;
        }

        public double Radius
        {
            get { return _radius; }
        }

        public double GetArea()
        {
            return Math.PI * _radius * _radius;
        }

        public string Name
        {
            get { return "Circle"; }
        }
    }

    public enum Color
    {
        Red,
        Green,
        Blue,
        Yellow = 10
    }

    public class Container<T>
    {
        private readonly List<T> _items = new List<T>();

        public void Add(T item)
        {
            _items.Add(item);
        }

        public T Get(int index)
        {
            return _items[index];
        }

        public int Count
        {
            get { return _items.Count; }
        }
    }

    public class Program
    {
        public static void Main(string[] args)
        {
            var calc = new Calculator();
            int sum = calc.Add(3, 5);
            Console.WriteLine($"Sum: {sum}");

            var circle = new Circle(5.0);
            double area = circle.GetArea();
            Console.WriteLine($"Area: {area}");

            var container = new Container<string>();
            container.Add("Hello");
            container.Add("World");

            foreach (var arg in args)
            {
                Console.WriteLine(arg);
            }

            for (int i = 0; i < 10; i++)
            {
                if (i % 2 == 0)
                {
                    Console.WriteLine(i);
                }
            }

            try
            {
                int result = calc.Factorial(10);
                Console.WriteLine(result);
            }
            catch (Exception ex)
            {
                Console.WriteLine(ex.Message);
            }
            finally
            {
                Console.WriteLine("Done");
            }

            // Switch statement
            Color color = Color.Red;
            switch (color)
            {
                case Color.Red:
                    Console.WriteLine("Red");
                    break;
                case Color.Green:
                    Console.WriteLine("Green");
                    break;
                default:
                    Console.WriteLine("Other");
                    break;
            }

            // Null coalescing
            string name = null;
            string display = name ?? "Unknown";

            // Conditional expression
            int x = sum > 10 ? 1 : 0;

            // Await (would need async context in real code)
            // var task = Task.Run(() => 42);
            // int val = await task;
        }
    }
}
