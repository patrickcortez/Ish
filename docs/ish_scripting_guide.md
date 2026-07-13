# Ish Scripting Guide

The Ish shell is built around a custom AST parsing engine and comes with its own intuitive scripting language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`, or pass a command directly to the interpreter using `ish -c "command here"`.

## Table of Contents
- [Core Syntax](#core-syntax)
- [Variables & Scoping](#variables--scoping)
- [Data Structures](#data-structures)
- [Math & Subshells](#math--subshells)
- [Control Flow](#control-flow)
- [Functions & Returns](#functions--returns)
- [Object-Oriented Programming (OOP)](#object-oriented-programming-oop)
- [Built-In Commands](#built-in-commands)

## Core Syntax

### Piping
Unlike standard POSIX shells, Ish allows using the colon `:` operator or the standard `|` for piping standard output between commands. More importantly, Ish pipes **JSON-structured data** (`IshValue`) under the hood, meaning objects stay structured!
```bash
> show . | str_tolower
# OR
> show . : str_tolower
```

### Sequential & Conditional Execution
Instead of using a semicolon `;`, Ish uses the explicit `then` keyword for continuous, sequential execution.
```bash
> create -d new_folder then change new_folder
```

Instead of `&&` and `||`, Ish natively encourages the natural language keywords `and then` and `or else`. (Standard `&&` and `||` are fully supported natively via the Tokenizer as well!)
```bash
> build_project and then run_tests or else out "Build Failed!"
> build_project && run_tests || out "Build Failed!"
```

**Comparison Operators**:
Ish supports robust comparison operators natively within the AST: `==`, `!=`, `<`, `>`, `<=`, `>=`.
```bash
> if ( $LAST == 0 ) { out "Success!" }
```

### Background Jobs
Instead of appending an ampersand `&`, use the explicit `job` keyword at the end of the command to send it to the background.
```bash
> cargo build --release job
```

### Redirection
Ish replaces obscure redirection operators with explicit, readable keywords:
- `to` maps to standard output overwrite (`>`)
- `append to` maps to standard output append (`>>`)
- `from` maps to standard input file read (`<`)
- `read doc` handles text stream injection (HereDoc / `<<`)
- `merge err` securely pipes standard error stream into standard output (`2>&1`)
- `DevNull` drops streams into the abyss (equivalent to `/dev/null`)

```bash
> out "Hello World" append to log.txt
> read from config.json
> mycmd merge err to output.log
> noisy_cmd to DevNull
```

### Parallel Execution
Ish uses the `while` operator to run the left node in the background simultaneously while the right node executes.
```bash
> long_running_task while short_task
```

## Data Structures

Ish internally supports strong typing with `Int`, `Float`, `Bool`, and `Null` values, alongside native strings.

## Variables & Scoping

Ish enforces strict variable management via the `let` keyword. 

### Strict Declaration
You **must** declare a new variable before assigning to it. Uninitialized variables or mutating undeclared variables will throw execution errors.

```bash
let name = "IshShell"
let count = 10
out "Running: $name"

count = 20  # Valid mutation
```

### Variable Scoping
Ish follows a block-scoped lifetime architecture:
- **Global Variables**: Declared at the root file level and accessible anywhere.
- **Local Variables**: Declared inside `if`, `while`, `for`, `foreach` blocks or `func` scopes. They are destroyed when the block ends.

> [!WARNING]
> Attempting to use a local variable outside its parent block will result in a **Linter Error**.

```bash
let global_var = "Visible everywhere"

if ( true ) {
    let local_var = "Only exists here"
    out $global_var   # Works!
}

out $local_var  # Error: Variable not defined!
```

### Arrays & Maps
Ish is fully Turing-complete with first-class data structure support that natively maps to JSON!

**Arrays**:
Arrays in Ish are strictly immutable. Initialize them natively using the `let[]` keyword and square brackets:
```bash
let[] my_arr = [ "apple", "banana", "cherry" ]
out "First item: $my_arr[0]"
```

**Lists**:
Lists are the mutable equivalents to arrays. You can instantiate them using the `new` keyword and invoke built-in methods like `add`, `remove`, and `clear`. You can also mutate indexes directly:
```bash
let my_list = new List()
my_list.add("apple")
my_list.add("banana")
my_list[1] = "blueberry"
my_list.remove(0)
my_list.clear()
```

**Maps**:
Initialize maps natively using the `Map` constructor keyword combined with JSON-like object notation. You can pass multiple key-value pair blocks separated by commas:
```bash
let my_map = Map(
    {"name": "Ish"},
    {"version": "1.0"}
)
out "Shell Name: $my_map[\"name\"]"
```

> [!IMPORTANT]
> **Strict Indexing**: When accessing elements from a Map or Array, the index must be explicitly quoted if it is a string (e.g., `["key"]`), a valid integer, or a variable (e.g., `[$var]`). Unquoted literal keys like `[key]` will throw an execution error.

## Math & Subshells

### Automatic Math Expressions
Ish natively evaluates math expressions directly within the syntax using grouping `( ... )`. Standard BODMAS operator precedence is respected (`*`, `/`, `+`, `-`).
You no longer need string expansions to perform math!

```bash
let result = ($a * $b + 2)
out $result
```

### Ternary Operators
Ish supports concise `if-else` assignments natively using the `? :` ternary operator syntax. It must be grouped as an expression.

```bash
let msg = ($result > 50) ? "big" : "small"
out $msg
```

### Subshells
You can execute and capture the output of commands or blocks natively using the `$( ... )` expansion syntax.

```bash
let files = "$(show .)"
out "Found files: $files"
```

**State Variables**:
- `$LAST`: Retrieves the numerical exit code of the most recently executed process.
- `$1`, `$2`, etc.: Retrieves command-line arguments passed directly to the script.

## Control Flow

### If/Else
Ish natively parses `if`, `elif`, and `else` blocks enclosed in curly braces `{}`.
```bash
if ( $status == "success" ) {
    out "Deploying..."
} elif ( $status == "pending" ) {
    out "Waiting..."
} else {
    out "Failed!"
}
```

### Loops
Iterate elegantly using `for` and `foreach` block structures. Loop evaluation seamlessly supports `break` and `continue`.
```bash
for (i = 0, $i < 5) {
    if ( $i == 3 ) { break }
    out "Iteration: $i"
}

foreach (item in $files) {
    if ( $item == "skip.txt" ) { continue }
    out "Processing $item"
}
```

### Try/Catch Error Handling
Ish features an integrated `try/catch` architecture. It safely catches any execution or unhandled errors block-scoped during evaluation:
```bash
try {
    dangerous_command
} catch {
    out "An error occurred: $ERROR"
}
```

## Functions & Returns

Functions allow you to encapsulate logic securely using the `func` keyword. Variables declared within a function are entirely isolated.

> [!IMPORTANT]
> **No Nested Declarations**: To enforce structural integrity, you cannot declare a `func`, `class`, `struct`, or `enum` inside a method body or any code block. Declarations must remain at the class or namespace level.

### Strict Returns
To enforce maintainable programming rules, the `return` statement has strict behavioral validation:
1. You can only return **raw values** (like `"hello"`, `5`) or **variables** (`$my_var`).
2. You **cannot** return commands directly. Expressions like `return out hello` will throw a linter error.
3. Subshells are permitted during a return statement **only if they yield a valid value**.

### Parameter Declarations
Parameters must be explicitly declared using `let` or `let[]`. You can also assign default values to arguments, which will be used if the caller doesn't provide them. 

For variadic arguments (accepting any number of trailing arguments), prefix the array parameter with the `params` keyword. Variadic parameters cannot have default values.

```bash
func calculate_sum(let a, let b = 5) {
    let sum = ($a + $b)
    return $sum
}

func log_messages(let prefix, params let[] messages) {
    foreach (msg in $messages) {
        out "$prefix: $msg"
    }
}

let result = calculate_sum(10)
out "The sum is: $result"
log_messages("INFO", "Starting up...", "Loading config...")
```

## Object-Oriented Programming (OOP)

Ish is deeply OOP-centric and strictly enforces a C#-style structure for robust programming. 

### Program Entry Point & Top-Level Constraints
Ish enforces extreme top-level strictness. **Absolutely no statements or variables can exist outside of a class, struct, enum, or with-import**.

Every valid Ish script **must** define a `public static class Program` with a `public static int Main(params let[] args)` method. When executing an Ish script, this `Main` method is the starting point of execution.

```bash
public static class Program {
    public static int Main(params let[] args) {
        out "Hello from the entry point!"
        
        let greeter = new Utilities::Greeter()
        greeter.say_hello("User")
        return 0
    }
}
```

### Classes, Structs, Enums, and Namespaces
You can organize your code using `namespace`, `class`, `struct`, and `enum`.

- **Namespace**: Used to logically group classes.
- **Class**: Defines an object with methods and properties.
- **Struct**: A lightweight data structure (behaves similarly to a class but typically used for pure data).
- **Enum**: Strongly-typed named constants.

### Importing Modules
To use classes defined in other files or directories, use the `with` keyword followed by the dot-separated path to the script (which mirrors C#'s `using`).
```bash
with src.utils.MathUtils;
```

```bash
namespace Utilities {
    public class Greeter {
        public func say_hello(name) {
            out "Hello, $name!"
        }
    }
}
```

### Access Specifiers
You can strictly control visibility using access specifiers:
- `public`: Accessible from anywhere.
- `private`: Accessible only within the declaring class.
- `protected`: Accessible within the declaring class and its subclasses.
- `internal`: Accessible within the same namespace.

### Static Members & Classes
Methods and properties can be marked as `static`, meaning they belong to the class itself rather than an instance. An entire class can also be declared as `static` ensuring it cannot be instantiated.

```bash
public static class MathUtil {
    public static func square(x) {
        return "$(( $x * $x ))"
    }
}
```
You can call static methods using the class name natively without instantiating it:
```bash
MathUtil.square(10)
```

### Enums
Enums allow you to define constant variants. They are accessed statically using dot notation:
```bash
public enum Status {
    Pending,
    InProgress,
    Completed
}

let current = Status.Pending
```

### Object Instantiation
To create an instance of a standard (non-static) class, use the `new` keyword. You can access properties and methods using dot notation (`.`).

```bash
let my_obj = new Utilities::Greeter()
my_obj.say_hello("World")
```

## Built-In Commands
- `change <path>`: Change the current working directory.
- `quit`: Exit the shell script / REPL.
- `let <var> = <val>`: Explicitly declare a variable.
- `out <text>`: Print to standard output.
- `cwd`: Print current working directory.
- `show [path]`: List directory contents natively mapped to an `IshValue::Array`.
- `read <file>`: Read file contents.
- `create <-f|-d> <name>`: Create file (`-f`) or directory (`-d`).
- `input [prompt]`: Wait for standard input string.
- `inputkey [prompt]`: Intercept exactly one keystroke natively.
- `jobs`: List all active background jobs.
- `fg <id>`: Bring a background job to the foreground and wait for it to complete.
- `kill <id>`: Forcefully terminate a running background job.
