# Ish Scripting Guide

The Ish shell is built around a custom AST parsing engine and comes with its own intuitive scripting language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`, or pass a command directly to the interpreter using `ish -c "command here"`.

## Table of Contents
- [Core Syntax](#core-syntax)
- [Data Structures](#data-structures)
- [Control Flow](#control-flow)
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

### Variables and Strict Declaration
Ish uses strict variable declarations. You **must** declare a new variable using the `declare` keyword. Attempting to mutate an undeclared variable throws a runtime execution error.

```bash
declare name = "IshShell"
declare count = 10
out "Running: $name"

count = 20  # Valid mutation
```

### Arrays & Maps
Ish is fully Turing-complete with first-class data structure support that natively maps to JSON!

**Arrays**:
Initialize natively using square brackets:
```bash
declare my_arr = [ "apple", "banana", "cherry" ]
out "First item: $my_arr[0]"
```

**Maps**:
Initialize maps natively using the `Map` constructor keyword:
```bash
declare my_map = Map("name", "Ish", "version", "1.0")
out "Shell Name: $my_map[name]"
```

### Automatic Math Expressions
Ish evaluates math expressions automatically using bodmas-compliant logic natively inside the AST.
```bash
declare x = 5 + 10 * 2
declare y = $x / 2.5
out $y
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

## Built-In Commands
- `change <path>`: Change the current working directory.
- `quit`: Exit the shell script / REPL.
- `declare <var> = <val>`: Explicitly declare a variable.
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
