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
> mkdir new_folder then cd new_folder
```

### Conditional Execution
Instead of `&&` and `||`, Ish natively encourages the natural language keywords `and then` and `or else`. However, standard `&&` and `||` are fully supported natively via the Tokenizer as aliases for user convenience!
```bash
> build_project and then run_tests or else echo "Build Failed!"
> build_project && run_tests || echo "Build Failed!"
```

**Comparison Operators**:
Ish supports robust comparison operators natively within the AST for conditional checks: `==`, `!=`, `<`, `>`, `<=`, `>=`.
```bash
> if ( $LAST == 0 ) { echo "Success!" }
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
> echo "Hello World" append to log.txt
> cat from config.json
> mycmd merge err to output.log
> noisy_cmd to DevNull
```

### Parallel Execution
Instead of using `&` to run nodes in parallel, Ish uses the `while` operator to run the left node in the background simultaneously while the right node executes.
```bash
> long_running_task while short_task
```

## Scripting Elements
Within a `.ish` script, you can leverage advanced programming capabilities natively evaluated by the Ish AST interpreter.

### Variables and Interpolation
Assign variables directly. Access them using the `$` prefix.
Ish supports **String Interpolation** out of the box. Variables embedded inside strings are evaluated automatically.
```bash
name="IshShell"
echo "Running: $name"
```

### Data Structures
Ish is fully Turing-complete with first-class data structure support.

**Arrays**:
Initialize natively using square brackets:
```bash
let my_arr = [ "apple", "banana", "cherry" ]
echo "First item: $my_arr[0]"
```

**Maps**:
Initialize maps natively using the `Map` constructor keyword:
```bash
let my_map = Map("name", "Ish", "version", "1.0")
echo "Shell Name: $my_map[name]"
```

### Math Expressions
Ish comes with a native recursive descent evaluator embedded as the `expr` internal utility for bodmas-compliant math!
```bash
let x = expr 5 + 10 * 2
echo $x
```

**State Variables**:
Ish provides built-in state variables automatically injected into the environment:
- `$LAST`: Retrieves the numerical exit code of the most recently executed process.
- `$1`, `$2`, etc.: Retrieves command-line arguments passed directly to the script.

### Control Flow
Ish natively parses `if`, `elif`, and `else` blocks enclosed in curly braces `{}`.
```bash
if ( $status == "success" ) {
    echo "Deploying..."
} elif ( $status == "pending" ) {
    echo "Waiting..."
} else {
    echo "Failed!"
}
```

### Loops
Iterate elegantly using `for` and `foreach` block structures. Loop evaluation seamlessly supports `break` to exit instantly and `continue` to jump to the next iteration.
```bash
for (i = 0, $i < 5) {
    if ( $i == 3 ) { break }
    echo "Iteration: $i"
}

foreach (item in $files) {
    if ( $item == "skip.txt" ) { continue }
    echo "Processing $item"
}
```

### Functions
Define reproducible native shell functions inside your scripts using the `fn` keyword.
```bash
fn build_and_deploy() {
    cargo build --release
    echo "Deployment Triggered"
}

build_and_deploy
```
