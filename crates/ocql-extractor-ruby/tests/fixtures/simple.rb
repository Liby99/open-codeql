# A simple Ruby test fixture for the extractor.

require 'json'
require_relative './helpers'

MAX_AGE = 100

# Module with classes
module Animals
  class Animal
    attr_reader :name, :age

    def initialize(name, age = 0)
      @name = name
      @age = age
    end

    def speak
      raise NotImplementedError, "Subclass must implement speak"
    end

    def name
      @name
    end

    def to_s
      "#{@name} (age: #{@age})"
    end

    def self.create(name, age: 0, **opts)
      new(name, age)
    end
  end

  class Dog < Animal
    def speak
      "Woof!"
    end

    def fetch(item, *extra_items, &callback)
      result = "Fetching #{item}"
      if callback
        callback.call(result)
      end
      result
    end
  end

  class Cat < Animal
    def speak
      "Meow!"
    end

    def purr
      return "Purrrr" if happy?
      nil
    end

    private

    def happy?
      true
    end
  end
end

# Various statements and expressions
def demonstrate
  dog = Animals::Dog.new("Rex", 3)
  cat = Animals::Cat.new("Whiskers", 5)

  # If/elsif/else
  if dog.age > cat.age
    puts "Dog is older"
  elsif dog.age == cat.age
    puts "Same age"
  else
    puts "Cat is older"
  end

  # Unless
  unless dog.nil?
    dog.speak
  end

  # While loop
  count = 0
  while count < 3
    puts count
    count += 1
  end

  # Until loop
  until count == 0
    count -= 1
  end

  # For loop
  for animal in [dog, cat]
    puts animal.to_s
  end

  # Case/when
  case dog.speak
  when "Woof!"
    puts "It's a dog"
  when "Meow!"
    puts "It's a cat"
  else
    puts "Unknown"
  end

  # Begin/rescue/ensure
  begin
    result = 10 / 0
  rescue ZeroDivisionError => e
    puts "Error: #{e.message}"
  ensure
    puts "Done"
  end

  # Block with do..end
  [1, 2, 3].each do |x|
    puts x
  end

  # Block with braces
  [4, 5, 6].map { |x| x * 2 }

  # Lambda
  doubler = ->(x) { x * 2 }
  doubler.call(5)

  # Ternary / conditional
  msg = dog.age > 2 ? "old" : "young"

  # Array and hash
  numbers = [1, 2, 3, 4, 5]
  config = { host: "localhost", port: 8080 }

  # Range
  range = 1..10

  # Symbols
  status = :active

  # Regex
  pattern = /\d+/

  # Splat and block argument
  def variadic(*args, &block)
    args.each(&block)
  end

  # Heredoc
  text = <<~HEREDOC
    Hello
    World
  HEREDOC

  # Yield
  def with_logging
    puts "Before"
    yield
    puts "After"
  end

  # Return, break, next, retry, redo
  return msg
end

# Top-level call
demonstrate
