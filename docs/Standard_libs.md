# Standard Libraries

The Standard Libraries in Ish provide cross-platform functionality built directly into the interpreter, exposed as static-style method calls on built-in provider names (e.g. `Str.ToLower(...)`, `FS.Exists(...)`) — the same call syntax used for your own classes' static methods. They return native `IshValue` types (`String`, `Int`, `Bool`, `Array`/`List` references, etc.) that map directly into Ish variables.

> **Note:** earlier versions of this document described these as `snake_case` shell-style commands (e.g. `fs_readfile`, `str_tolower`) invoked bash-style. That syntax no longer exists — Ish is a compiled OOP language now (see the [Scripting Guide](/docs/ish_scripting_guide.md)), and every standard library is called as `ModuleName.Method(args)`.

## Table of Contents
- [CommandLine I/O (CommandLine)](#commandline-io)
- [File System I/O (FS)](#file-system-io)
- [Networking (Net)](#networking)
- [String Utilities (Str)](#string-utilities)
- [Math (Math)](#math)
- [Date and Time (Time)](#date-and-time)
- [Machine / OS (OS)](#machine--os)
- [External Processes (ExtProc)](#external-processes)
- [Namespaces & Imports](#namespaces--imports)

---

## CommandLine I/O
Name: **CommandLine**

The primary way to read from and write to the terminal.

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `OutputLine` | `[value]` | Writes a value followed by a newline. Writes a blank line if called with no args. | `null` |
| `Output` | `<value>` | Writes a value with no trailing newline. | `null` |
| `Input` | None | Reads a full line from stdin. | `string` |
| `Read` | None | Reads a single keypress from stdin. | `string` |
| `ForeColor` | `<color>` | Sets the terminal foreground color. Accepts a name (`red`, `green`, `blue`, `black`, `yellow`, `magenta`, `cyan`, `white`) or a hex string. | `null` |
| `BackColor` | `<color>` | Sets the terminal background color. Same accepted values as `ForeColor`. | `null` |
| `ResetColor` | None | Resets terminal colors to default. | `null` |

```csharp
CommandLine.OutputLine("Hello, world!");
let name = CommandLine.Input();
```

---

## File System I/O
Name: **FS**

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `Exists` | `<path>` | Checks whether a file or directory exists. | `bool` |
| `ReadAllLines` | `<path>` | Reads a file, one entry per line. | Array of `string` |
| `ReadAllText` | `<path>` | Reads an entire file as a single string. | `string` |
| `WriteAllText` | `<path> <data> [append]` | Writes `data` to a file. `append` (bool) defaults to `false` (overwrite). | `bool` |
| `List` | `<path>` | Lists the contents of a directory. | Array of `string` |

> **Note:** earlier documentation additionally listed `fs_createfile`, `fs_deletefile`, `fs_createdir`, `fs_deletedir`, `fs_copy`, `fs_move`, `fs_getfileperm`, and `fs_getdirperm`. Those do not currently exist as `FS` methods — if you need them, use [`ExtProc`](#external-processes) to shell out to the OS in the meantime.

```csharp
if (FS.Exists("data.txt")) {
    let content = FS.ReadAllText("data.txt");
    CommandLine.OutputLine(content);
}
```

---

## Networking
Name: **Net**

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `Get` | `<url>` | Sends an HTTP GET request. | `string` |
| `Post` | `<url> <data>` | Sends an HTTP POST request. | `string` |
| `Download` | — | Downloads a resource. | `bool` |

> **Status:** as of the current build, `Net.Get` and `Net.Post` are stubs that return a placeholder "not implemented natively yet" string, and `Net.Download` always returns `false`. Earlier documentation described these as fully functional (including a separate `net_getsecure`, `net_ping`, `net_ssid`, `net_ip`, and `net_resolve`, none of which currently exist). Treat `Net` as unimplemented for now — this section will need another pass once networking lands for real.

---

## String Utilities
Name: **Str**

Static, module-style string helpers. Every string value also exposes an equivalent set of instance methods directly (e.g. `myStr.ToUpper()`) plus in-place mutation methods (`.Append()`, `.Clear()`) — see the [Scripting Guide](/docs/ish_scripting_guide.md#strings--characters) for those and for the newer `char` type.

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `ToLower` | `<string>` | Converts to lowercase. | `string` |
| `ToUpper` | `<string>` | Converts to uppercase. | `string` |
| `Reverse` | `<string>` | Reverses the string. | `string` |
| `Trim` | `<string>` | Trims whitespace from both ends. | `string` |
| `TrimStart` | `<string>` | Trims leading whitespace. | `string` |
| `TrimEnd` | `<string>` | Trims trailing whitespace. | `string` |
| `Length` | `<string>` | Returns the length of the string. | `int` |
| `Contains` | `<string> <sub>` | Checks whether `string` contains `sub`. | `bool` |
| `IndexOf` | `<string> <sub>` | Index of first occurrence of `sub`, or `-1`. | `int` |
| `Substring` | `<string> <start> [end]` | Substring from index `start` to `end` (exclusive). | `string` |
| `Join` | `<array> <separator>` | Joins an array of strings with `separator`. | `string` |
| `Split` | `<string> <separator>` | Splits a string on `separator`. | Array of `string` |
| `Replace` | `<string> <old> <new>` | Replaces all occurrences of `old` with `new`. | `string` |

> **Note:** `Str.Substring(string, start, end)` takes an *end index*, whereas the newer instance method `str.Substring(start, count)` takes a *length* — they are not interchangeable. `str_find` and `str_len` from earlier documentation are now `IndexOf` and `Length` respectively.

```csharp
let shout = Str.ToUpper("hello");
CommandLine.OutputLine(shout); // HELLO
```

---

## Math
Name: **Math**

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `Abs` | `<n>` | Absolute value. | `float` |
| `Ceiling` | `<n>` | Rounds up. | `float` |
| `Floor` | `<n>` | Rounds down. | `float` |
| `Round` | `<n>` | Rounds to nearest integer. | `float` |
| `Pow` | `<n> <exp>` | Raises `n` to the power `exp`. | `float` |
| `Min` | `<a> <b>` | Smaller of the two values. | `float` |
| `Max` | `<a> <b>` | Larger of the two values. | `float` |
| `Sqrt` | `<n>` | Square root. | `float` |

All `Math` results are returned as `float`, even for integer-looking inputs.

```csharp
let area = Math.Round(Math.Pow(radius, 2) * 3.14159);
```

---

## Date and Time
Name: **Time**

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `Now` | None | Current Unix timestamp (seconds). | `int` |
| `Unix` | None | Alias for `Now`. | `int` |
| `Format` | `<value>` | Formats a value. | `string` |

> **Status:** `Time.Format` is currently a stub that returns `"Formatted: <value>"` rather than performing real date formatting, and there is no `Time.Parse`. Earlier documentation described a fully working `time_format`/`time_parse` pair with format-string support — that hasn't landed yet.

---

## Machine / OS
Name: **OS**

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `GetEnv` | `<name>` | Gets an environment variable's value (`null` if unset). | `string` or `null` |
| `SetEnv` | `<name> <value>` | Sets an environment variable for the running process. | `bool` |
| `Platform` | None | Operating system name (e.g. `linux`, `windows`, `macos`). | `string` |
| `Arch` | None | CPU architecture (e.g. `x86_64`). | `string` |
| `Cwd` | None | Current working directory. | `string` |

> **Note:** earlier documentation additionally listed `os_hostname`, `os_getenvvars` (list all), `os_version`, `os_exit`, `os_sleep`, `os_clear`, and `os_users`. None of those currently exist as `OS` methods.

---

## External Processes
Name: **ExtProc**

New standard library module for spawning and running external programs, added alongside the IO layer rework. Not present in earlier documentation.

| Method | Arguments | Description | Returns |
|---|---|---|---|
| `Start` | `<program> [args]` | Runs `program` to completion, optionally passing an array of string arguments. | Object with `ExitCode` (`int`), `StandardOutput` (`string`), `StandardError` (`string`) |

```csharp
let[] flags = ["-la"];
let result = ExtProc.Start("ls", flags);
CommandLine.OutputLine(result.StandardOutput);
CommandLine.OutputLine($"Exit code: {result.ExitCode}");
```

---

## Namespaces & Imports

You can no longer merge another script's functions into scope by file path with `with sample_lib.ish`. Instead, `with` imports **by namespace name** — see [Namespaces & Imports](/docs/ish_scripting_guide.md#namespaces--imports) in the Scripting Guide for the current behavior and an example.
