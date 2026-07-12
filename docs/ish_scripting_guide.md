# Ish Scripting Guide

The Ish shell is built around a custom AST parsing engine and comes with its own intuitive scripting language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`, or pass a command directly to the interpreter using `ish -c "command here"`.

## Table of Contents
- [Core Syntax](#core-syntax)
- [Variables & Scoping](#variables--scoping)
- [Data Structures](#data-structures)
- [Math & Subshells](#math--subshells)
- [Control Flow](#control-flow)
- [Functions & Returns](#functions--returns)
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
Initialize natively using square brackets:
```bash
let my_arr = [ "apple", "banana", "cherry" ]
out "First item: $my_arr[0]"
```

**Maps**:
Initialize maps natively using the `Map` constructor keyword:
```bash
let my_map = Map("name", "Ish", "version", "1.0")
out "Shell Name: $my_map[name]"
```

## Math & Subshells

### Automatic Math Expressions
Ish evaluates math expressions automatically using bodmas-compliant logic inside the `$(( ... ))` expansion syntax.

```bash
let x = "$(( 5 + 10 * 2 ))"
let y = "$(( $x / 2.5 ))"
out $y
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

### Strict Returns
To enforce maintainable programming rules, the `return` statement has strict behavioral validation:
1. You can only return **raw values** (like `"hello"`, `5`) or **variables** (`$my_var`).
2. You **cannot** return commands directly. Expressions like `return out hello` will throw a linter error.
3. Subshells are permitted during a return statement **only if they yield a valid value**.

```bash
func calculate_sum(a, b) {
    let sum = "$(( $a + $b ))"
    return $sum
}

let result = calculate_sum(10, 20)
out "The sum is: $result"
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
