# Ish Shell Guide

Welcome to the **Ish (Intelli-Shell)** User Guide! Ish is a cross-platform system shell written in Rust. It is designed to be highly efficient, bridging the gap between raw unstructured terminal output and modern structured data objects.

## Table of Contents
- [JSON-Structured Output](#json-structured-output)
- [Piping Objects](#piping-objects)
- [Built-In Commands](#built-in-commands)
- [Advanced Shell Logic](#advanced-shell-logic)

## JSON-Structured Output

Ish handles data beautifully using native tables instead of messy text. Under the hood, Ish passes variables as JSON-like structures (Strings, Integers, Booleans, Arrays, and Maps).

Whenever an Ish command (or an external command) outputs an Array of Maps, Ish automatically intercepts the data and renders it as a structured Unicode table.

```bash
> show .
┌────────┬─────────┬────────────────────┬───────┐
│ is_dir ┆ is_file ┆ name               ┆ size  │
╞════════╪═════════╪════════════════════╪═══════╡
│ false  ┆ true    ┆ .gitignore         ┆ 45    │
├╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌┤
│ true   ┆ false   ┆ src                ┆ 0     │
└────────┴─────────┴────────────────────┴───────┘
```

> If an external tool like `curl` outputs a JSON array, Ish automatically converts it to a table seamlessly!

## Piping Objects

To pipe commands in Ish, you can use either the standard Unix pipe `|` or the Ish specific colon pipe `:`.

```bash
> show . | str_tolower
# OR
> show . : str_tolower
```

When you pipe commands in Ish, you are not just passing text strings. Ish serializes the structured data into JSON and pipes it directly. This means you can interact with complex objects securely inside the shell.

## Built-In Commands

Ish handles normal OS commands natively, but also provides internal built-ins:

- `change <path>`: Changes the current working directory.
- `quit`: Gracefully terminates the shell.
- `let <var> = <val>`: Explicitly declares a local or global variable.
- `out <text>`: Prints text to standard output.
- `cwd`: Prints the current working directory.
- `show [path]`: Lists directory contents natively as structured tabular objects.
- `read <file>`: Reads and prints file contents.
- `create <-f|-d> <name>`: Creates a file (`-f`) or a directory (`-d`). Defaults to file.
- `input [prompt]`: Reads a full line of text from standard input.
- `inputkey [prompt]`: Reads exactly one keystroke from standard input in raw mode.
- `expr <math_expression>`: Evaluates mathematical expressions automatically.

## Advanced Shell Logic

Because Ish uses the exact same parsing engine for both the interactive prompt and `.ish` scripts, **everything available in Ish Scripts is available live in the interactive shell!**

You can use English redirections (`merge err`, `append to`), control flow, and mathematical operators dynamically. For full syntax details, see the [Ish Scripting Guide](ish_scripting_guide.md).
