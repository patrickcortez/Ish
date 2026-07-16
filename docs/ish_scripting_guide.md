# Ish Programming Language Guide

Welcome to the **Ish Programming Language**! 

Ish has evolved into a fully robust, compiled Object-Oriented Programming (OOP) language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`.

## Table of Contents
- [Core Syntax](#core-syntax)
- [Variables & Scoping](#variables--scoping)
- [Data Structures](#data-structures)
- [Strings & Characters](#strings--characters)
- [Control Flow](#control-flow)
- [Error Handling](#error-handling)
- [Functions & Returns](#functions--returns)
- [Object-Oriented Programming (OOP)](#object-oriented-programming-oop)
- [Enums](#enums)
- [Generics & Advanced Types](#generics--advanced-types)
- [Namespaces & Imports](#namespaces--imports)
- [Memory Management](#memory-management)

---

## Core Syntax

### Program Entry Point
Every Ish script must have an entry point defined in a `Program` class. The class must be `public static`, and it must contain a `public static` method named exactly **`Main`** (capital M) with exactly one parameter: `params string[] args`. This is stricter than it may look — the interpreter rejects any other shape (no lowercase `main`, no empty parameter list, no other parameter name or type).

```csharp
public static class Program {
    public static func Main(params string[] args) {
        CommandLine.OutputLine("Hello World!");
    }
}
```

`args` is populated automatically from the command-line arguments passed to `ish myscript.ish arg1 arg2` — you don't fill it in yourself.

### String Interpolation
Ish supports string interpolation using the `$"..."` syntax. You can embed expressions directly inside strings using curly braces `{}`.

```csharp
let name = "Ish";
let version = 1.0;
CommandLine.OutputLine($"Welcome to {name} v{version}!");
```

---

## Variables & Scoping

Ish enforces strict variable management via the `let` keyword (or any other type-like word — see note below). Note that as of the current build, Ish no longer uses a `$` prefix for variable references anywhere in the language; writing `$name` will raise a parse error.

### Strict Declaration
You **must** declare a new variable before assigning to it. 
```csharp
let name = "Ish Language";
let count = 10;
count = 20;  // Valid mutation
```

> **Implementation note:** `let` isn't a hard-coded keyword — the parser accepts *any* identifier in that leading "type" position (`let`, `var`, `string`, `int`, `float`, `bool`, `char`, `List<T>`, or even a class name) and treats it as an optional type annotation before the variable name. `let` is simply the idiomatic convention used throughout Ish code; it carries no special enforcement beyond that of any other type word.

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
Attempting to assign into an array index (`my_arr[0] = "x"`) raises a runtime error telling you to use a `List` instead — see [Generics & Advanced Types](#generics--advanced-types).

### Maps
Initialize maps natively using the `Map` constructor.
```csharp
let my_map = Map(
    {"name": "Ish"},
    {"version": "1.0"}
);
CommandLine.OutputLine(my_map["name"]);
```
Unlike arrays, indexed assignment into a `Map` (`my_map["name"] = "New"`) is allowed and updates/inserts the entry in place.

---

## Strings & Characters

### The `char` Type
Ish has a dedicated `char` type. Single-quoted literals (`'a'`) are now always parsed as a `char`, not a one-character string — double quotes (`"a"`) are the only way to write a string literal. Standard escape sequences are supported inside a char literal: `'\n'`, `'\r'`, `'\t'`, `'\\'`, `'\''`, `'\"'`, `'\0'`.

```csharp
char initial = 'A';
CommandLine.OutputLine(initial.ToUpper()); // A
```

`char` instance methods:

| Method | Description | Returns |
|---|---|---|
| `.IsLetter()` | Is the character alphabetic? | `bool` |
| `.IsDigit()` | Is the character an ASCII digit? | `bool` |
| `.IsWhiteSpace()` | Is the character whitespace? | `bool` |
| `.IsAlnum()` / `.IsLetterOrDigit()` | Is the character alphanumeric? | `bool` |
| `.ToLower()` | Lowercase version | `char` |
| `.ToUpper()` | Uppercase version | `char` |

### Indexing a String Returns a Char
Indexing into a string with `str[i]` now returns a single `char`, not a substring:
```csharp
let word = "Ish";
let firstLetter = word[0]; // 'I' as a char, not a 1-character string
```

Iterating a string with `foreach` also now walks its characters one at a time (rather than splitting on whitespace, as older builds did):
```csharp
foreach (c in "Hi!") {
    CommandLine.OutputLine(c); // prints H, i, ! on separate lines
}
```

### String Instance Methods
Strings support instance-style method calls directly, in addition to the `Str.*` static helpers documented in [Standard_libs.md](/docs/Standard_libs.md):

| Method | Description | Returns |
|---|---|---|
| `.Substring(start, count)` | Substring starting at `start`, `count` characters long (omit `count` for "to the end") | `string` |
| `.IndexOf(sub)` | Index of first occurrence of `sub` (`string` or `char`), or `-1` | `int` |
| `.Contains(sub)` | Whether the string contains `sub` | `bool` |
| `.ToLower()` | Lowercased copy | `string` |
| `.ToUpper()` | Uppercased copy | `string` |
| `.Trim()` | Trims leading/trailing whitespace | `string` |
| `.RemoveSubstring(sub)` | Removes all occurrences of `sub` | `string` |
| `.Length()` | Character count | `int` |

> **Note on signature differences:** the instance `.Substring(start, count)` takes a *length*, while the static `Str.Substring(str, start, end)` (see Standard Libraries) takes an *end index*. Double-check which form you're calling.

### Mutable Strings
Three instance methods mutate the string variable **in place**, rather than returning a new value — a change from earlier builds where strings behaved as fully immutable:

| Method | Description |
|---|---|
| `.Append(value)` | Appends `value` to the string in place |
| `.AppendTo(value)` | Alias for `.Append(value)` |
| `.Clear()` | Empties the string in place |

```csharp
let log = "Startup: ";
log.Append("connecting...");
CommandLine.OutputLine(log); // Startup: connecting...
log.Clear();
CommandLine.OutputLine(log); // (empty)
```

This mutation only works when the method is called directly on a variable holding a string (e.g. `myVar.Append(x)`) — it does not apply to arbitrary expressions.

### Static `string` Helpers
The bare `string` type itself exposes a few static members:

| Member | Description | Returns |
|---|---|---|
| `string.Empty` | The empty string | `string` |
| `string.IsNullOrWhiteSpace(val)` | True if `val` is null or only whitespace | `bool` |
| `string.Join(arrayOrList, sep)` | Joins elements with `sep` | `string` |
| `string.Concat(...)` | Concatenates all arguments (or all elements of a single array/list argument) | `string` |

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

### Ternary Operator
For simple conditional expressions, Ish supports a `? :` ternary operator:
```csharp
let status = (age >= 18) ? "adult" : "minor";
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
Remember: `foreach` over a `string` iterates `char` values, not words — see [Strings & Characters](#strings--characters).

---

## Error Handling

Ish supports structured error handling with `try`/`catch`. The caught error is bound to a variable name you can reference in the `catch` block (it defaults to `err` if you don't name one):

```csharp
try {
    let result = 10 / 0;
    CommandLine.OutputLine("This won't print");
} catch err {
    CommandLine.OutputLine($"Caught an error: {err}");
}
```

---

## Functions & Returns

Functions are declared using the `func` keyword. **Every parameter requires a leading type-like word** (`let`, `var`, `string`, `int`, etc.) — a bare parameter name with nothing before it (`func add(a, b)`) fails to parse with "Method parameters must have a type specifier".

```csharp
public func add(let a, let b) {
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

### Constructors and Destructors
A method whose name matches the class name acts as its constructor; a method named `~ClassName` acts as its destructor, invoked when the Gobbler reclaims the instance.

```csharp
public class Person {
    let name = "";
    let age = 0;

    public func Person(let name, let age) {
        this.name = name;
        this.age = age;
    }

    public func introduce() {
        CommandLine.OutputLine($"Hi, I am {this.name}.");
    }
}

// Structs support the same pattern
public struct Point {
    let x = 0;
    let y = 0;

    public func Point(let x, let y) {
        this.x = x;
        this.y = y;
    }

    public func ~Point() {
        CommandLine.OutputLine("Point destroyed!");
    }
}
```

---

## Enums

Ish supports simple `enum` types, declared with `enum` and a `{ ... }` block of comma-separated variant names. Variants resolve to zero-based integers:

```csharp
public enum Status {
    Pending,
    Active,
    Closed
}

let s = Status.Active; // 1
```

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

## Namespaces & Imports

Group related classes, structs, and enums with `namespace`:

```csharp
namespace App {
    public class Person {
        // ...
    }
}
```

To use declarations from a namespace defined in a different `.ish` file in the same project, import it **by namespace name** with `with`:

```csharp
with App;
```

This is not a file-path import. At startup, Ish recursively scans every `.ish` file in the current working directory tree, finds whichever file(s) declare `namespace App { ... }`, and merges their classes/structs/enums/functions into your program. If no file in the project declares the named namespace, Ish reports an error at runtime rather than failing to parse.

---

## Memory Management

Ish uses a fully automatic **Memory Management System (MMS)** called **The Gobbler**.

### Automatic Garbage Collection
You never have to manually free memory in Ish. The Gobbler actively traces your variables. When an object goes out of scope, the Gobbler instantly unallocates its memory using a rigorous **Mark-and-Sweep** algorithm, invoking any destructor (`~ClassName`) defined on the object's class along the way.
