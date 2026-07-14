# Ish Programming Language Guide

Welcome to the **Ish Programming Language**! 

Ish has evolved into a fully robust, compiled Object-Oriented Programming (OOP) language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`.

## Table of Contents
- [Core Syntax](#core-syntax)
- [Variables & Scoping](#variables--scoping)
- [Data Structures](#data-structures)
- [Control Flow](#control-flow)
- [Functions & Returns](#functions--returns)
- [Object-Oriented Programming (OOP)](#object-oriented-programming-oop)
- [Generics & Advanced Types](#generics--advanced-types)
- [Memory Management](#memory-management)

---

## Core Syntax

### Program Entry Point
Every Ish script must have an entry point defined in a `Program` class.
```csharp
public static class Program {
    public static func main() {
        CommandLine.OutputLine("Hello World!");
    }
}
```

### String Interpolation
Ish supports string interpolation using the `$"..."` syntax. You can embed expressions directly inside strings using curly braces `{}`.

```csharp
let name = "Ish";
let version = 1.0;
CommandLine.OutputLine($"Welcome to {name} v{version}!");
```

---

## Variables & Scoping

Ish enforces strict variable management via the `let` keyword. 

### Strict Declaration
You **must** declare a new variable before assigning to it. 
```csharp
let name = "Ish Language";
let count = 10;
count = 20;  // Valid mutation
```

### Variable Scoping
Ish follows a block-scoped lifetime architecture:
- **Global Variables**: Declared at the root file level and accessible anywhere.
- **Local Variables**: Declared inside `if`, `while`, `for`, `foreach` blocks or `func` scopes. They are destroyed when the block ends.

---

## Data Structures

Ish is fully Turing-complete with first-class data structure support!

### Arrays
Arrays in Ish are strictly immutable. Initialize them natively using the `let[]` keyword and square brackets:
```csharp
let[] my_arr = [ "apple", "banana", "cherry" ];
CommandLine.OutputLine(my_arr[0]);
```

### Maps
Initialize maps natively using the `Map` constructor.
```csharp
let my_map = Map(
    {"name": "Ish"},
    {"version": "1.0"}
);
CommandLine.OutputLine(my_map["name"]);
```

---

## Control Flow

### If/Else Statements
Ish supports standard conditional branching.
```csharp
if (count > 10) {
    CommandLine.OutputLine("Greater than 10");
} else if (count == 10) {
    CommandLine.OutputLine("Exactly 10");
} else {
    CommandLine.OutputLine("Less than 10");
}
```

### Switch Statements
Ish features a powerful `switch` statement for cleaner multi-condition branching.
```csharp
let value = 2;
switch (value) {
    case 1: {
        CommandLine.OutputLine("One");
        break;
    }
    case 2: {
        CommandLine.OutputLine("Two");
        break;
    }
    default: {
        CommandLine.OutputLine("Other");
        break;
    }
}
```

### Loops
Ish supports `while`, `for`, and `foreach` loops.
```csharp
// For Loop
for (let i = 0; i < 5; i = i + 1) {
    CommandLine.OutputLine($"Iter: {i}");
}

// Foreach Loop
let[] fruits = ["apple", "banana"];
foreach (fruit in fruits) {
    CommandLine.OutputLine($"Fruit: {fruit}");
}
```

---

## Functions & Returns

Functions are declared using the `func` keyword.
```csharp
public func add(a, b) {
    return (a + b);
}

let result = add(5, 10);
```

---

## Object-Oriented Programming (OOP)

Ish natively supports **Classes**, **Structs**, **Inheritance**, and **Static Methods**.

### Classes and Inheritance
Classes are declared using the `class` keyword. You can inherit from another class using the `:` operator. 
Methods can be called natively using strict parenthesis `()` syntax.

```csharp
public class Animal {
    public func Speak() {
        CommandLine.OutputLine("Animal makes a sound.");
    }
}

// Dog inherits from Animal
public class Dog : Animal {
    public func Bark() {
        CommandLine.OutputLine("Woof!");
    }
}

// Usage
let dog = new Dog();
dog.Speak(); // Inherited from Animal!
dog.Bark();
```

> **How it works internally**: When `dog.Speak()` is called, the executor traverses up the inheritance chain checking `Dog`'s methods, and then its base class `Animal`, successfully resolving and executing the function.

---

## Generics & Advanced Types

Ish fully supports **Generics** allowing strongly-typed, reusable object instantiations like `List<T>`.

### The List Object
Lists are mutable arrays. You can instantiate them using the `new` keyword and invoke built-in methods like `add`, `remove`, and `clear`. 

```csharp
let my_list = new List<string>();
my_list.add("apple");
my_list.add("banana");
my_list.remove(0); // Removes "apple"
my_list.clear();
```

> **How it works internally**: The parser specifically extracts the generic `<T>` types for syntax validation. The backend memory manager automatically strips generic parameters during instantiation to map it efficiently into a dynamically sized `List` structure on the heap.

---

## Memory Management

Ish uses a fully automatic **Memory Management System (MMS)** called **The Gobbler**.

### Automatic Garbage Collection
You never have to manually free memory in Ish. The Gobbler actively traces your variables. When an object goes out of scope, the Gobbler instantly unallocates its memory using a rigorous **Mark-and-Sweep** algorithm. 
