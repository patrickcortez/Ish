# Ish Scripting Guide

The Ish shell is built around a custom AST parsing engine and comes with its own intuitive scripting language. You can run scripts headlessly directly from your terminal using `ish myscript.ish`, or pass a command directly to the interpreter using `ish -c "command here"`.

## Core Syntax

### Piping
Unlike standard POSIX shells, Ish uses the colon `:` operator for piping standard output between commands.
```bash
> ls : grep "txt"
```
*(Note: If executing pure native commands on Windows, Ish will map the `:` pipeline operator directly to the PowerShell `|` operator for `.NET` object preservation).*

### Sequential Execution
Instead of using a semicolon `;`, Ish uses the explicit `then` keyword for continuous, sequential execution.
```bash
> create -d new_folder then change new_folder
```

### Conditional Execution
Instead of `&&` and `||`, Ish natively encourages the natural language keywords `and then` and `or else`. However, standard `&&` and `||` are fully supported natively via the Tokenizer as aliases for user convenience!
```bash
> build_project and then run_tests or else out "Build Failed!"
> build_project && run_tests || out "Build Failed!"
```

**Comparison Operators**:
Ish supports robust comparison operators natively within the AST for conditional checks: `==`, `!=`, `<`, `>`, `<=`, `>=`.
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
Instead of using `&` to run nodes in parallel, Ish uses the `while` operator to run the left node in the background simultaneously while the right node executes.
```bash
> long_running_task while short_task
```

## Built-In Commands
Ish ships with cross-platform native commands evaluated internally without launching external processes:
- `change <path>`: Change the current working directory.
- `quit`: Exit the shell script / REPL.
- `declare <var> = <val>`: Explicitly declare a variable.
- `out <text>`: Print to standard output.
- `cwd`: Print current working directory.
- `show [path]`: List directory contents.
- `read <file>`: Read file contents.
- `create <-f|-d> <name>`: Create file (`-f`) or directory (`-d`).
- `input [prompt]`: Wait for standard input string.
- `inputkey [prompt]`: Intercept exactly one keystroke natively.
- `jobs`: List all active background jobs.
- `fg <id>`: Bring a background job to the foreground and wait for it to complete.
- `kill <id>`: Forcefully terminate a running background job.

## Scripting Elements
Within a `.ish` script, you can leverage advanced programming capabilities natively evaluated by the Ish AST interpreter.

### Variables, Types and Strict Declaration
Ish uses strict variable declarations. You **must** declare a new variable using the `declare` keyword. Attempting to mutate a variable that hasn't been declared will result in a runtime execution error.
Ish internally supports strong typing with `Int`, `Float`, `Bool`, and `Null` values, alongside native strings.

```bash
declare name = "IshShell"
declare count = 10
out "Running: $name"

count = 20  # Valid mutation
undeclared = 50 # Invalid: Throws an Execution Error
```

### Data Structures
Ish is fully Turing-complete with first-class data structure support.

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
Ish evaluates math expressions automatically using bodmas-compliant logic natively inside the AST without needing an `expr` utility. It handles types (Int and Float) safely!
```bash
declare x = 5 + 10 * 2
declare y = $x / 2.5
out $y
```

**State Variables**:
Ish provides built-in state variables automatically injected into the environment:
- `$LAST`: Retrieves the numerical exit code of the most recently executed process.
- `$1`, `$2`, etc.: Retrieves command-line arguments passed directly to the script.

### Control Flow
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
Iterate elegantly using `for` and `foreach` block structures. Loop evaluation seamlessly supports `break` to exit instantly and `continue` to jump to the next iteration.
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
    declare sum = 10 + 20
    undeclared_var = 50
} catch $err {
    out "An error occurred: "
    out $err
}
```

### Functions
Define reproducible native shell functions inside your scripts using the `fn` keyword.
```bash
fn build_and_deploy() {
    cargo build --release
    out "Deployment Triggered"
}

build_and_deploy
```

### Advanced Linting & Error Reporting
Ish features a professional-grade syntax linter built directly into the parser. If your script contains any syntax errors (such as missing loop bodies, unclosed functions, or mismatched pipelines), the linter will safely catch the error and provide a precise line and column location to help you fix it:
```bash
Linter Error at Line 14, Column 5: 'if' statement has an empty body
```
