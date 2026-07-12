# Standard Libraries

The Standard Libraries in Ish provide cross-platform functionality built directly into the interpreter. These commands return structured JSON-compatible data (like `IshValue::Array` or `IshValue::String`) seamlessly mapping to the native variable system.

## Table of Contents
- [File System I/O (IshFS)](#file-system-io)
- [Networking (IshNet)](#networking)
- [String Utilities (IshStr)](#string-utils)
- [Date and Time (IshTime)](#date-and-time)
- [Machine (IshOS)](#machine)
- [User Libraries](#user-libraries)

---

## File System I/O
Name: **IshFS**

| Command | Arguments | Description | Returns |
|---|---|---|---|
| `fs_readfile` | `<path>` | Reads a file line by line. | Array of Strings |
| `fs_writefile` | `<path> <data> [append]` | Writes data to a file. Append is true by default. | Exit Code |
| `fs_createfile` | `<path>` | Creates an empty file. | Exit Code |
| `fs_deletefile` | `<path>` | Deletes a file. | Exit Code |
| `fs_exists` | `<path>` | Checks if a file or directory exists. | Boolean (True/False) |
| `fs_createdir` | `<path>` | Creates a directory. | Exit Code |
| `fs_deletedir` | `<path>` | Deletes a directory. | Exit Code |
| `fs_list` | `<path>` | Lists contents of a directory. | Array of Strings |
| `fs_copy` | `<src> <dest>` | Copies a file or folder. | Exit Code |
| `fs_move` | `<src> <dest>` | Moves or renames a file or folder. | Exit Code |
| `fs_getfileperm`| `<path>` | Gets file permissions in octal format. | String |
| `fs_getdirperm` | `<path>` | Gets directory permissions in octal format.| String |

---

## Networking
Name: **IshNet**

| Command | Arguments | Description | Returns |
|---|---|---|---|
| `net_isavailable` | None | Checks if internet is available. | Exit Code (0=Yes, 1=No)|
| `net_ssid` | None | Returns the SSID of the current network. | String |
| `net_ip` | None | Returns IP addresses of the current network. | Array of Strings |
| `net_ping` | `<host>` | Pings a host (domain or IP). | Exit Code (0=Yes, 1=No)|
| `net_get` | `<url> [headers]` | Sends HTTP GET request. | String (Response Body)|
| `net_post` | `<url> <data> [headers]`| Sends HTTP POST request. | String (Response Body)|
| `net_resolve` | `<domain>` | Resolves a domain to an IP address. | String |
| `net_getsecure` | `<url> [headers]` | Identical to `net_get` but strictly HTTPS. | String |

---

## String Utils
Name: **IshStr**

| Command | Arguments | Description | Returns |
|---|---|---|---|
| `str_tolower` | `<string>` | Converts string to lowercase. | String |
| `str_toupper` | `<string>` | Converts string to uppercase. | String |
| `str_substr` | `<string> <start> [end]`| Returns a substring from index `start` to `end`. | String |
| `str_join` | `<array> <separator>` | Joins an array of strings into a single string. | String |
| `str_split` | `<string> <separator>` | Splits a string using the separator. | Array of Strings |
| `str_replace` | `<string> <old> <new>` | Replaces all occurrences of `old` with `new`. | String |
| `str_contains` | `<string> <sub_str>`| Checks if string contains the substring. | Boolean |
| `str_find` | `<string> <sub_str>`| Returns index of first occurrence. | Integer |
| `str_len` | `<string>` | Returns the length of the string. | Integer |
| `str_reverse` | `<string>` | Reverses the string. | String |
| `str_trim` | `<string>` | Trims whitespace from both ends. | String |

---

## Date and Time
Name: **IshTime**

| Command | Arguments | Description | Returns |
|---|---|---|---|
| `time_now` | None | Returns the current date and time. | String |
| `time_unix` | None | Returns the current unix time. | Integer |
| `time_format` | `<timestamp> <format>` | Formats a unix timestamp or RFC3339 string. | String |
| `time_parse` | `<string> <format>`| Parses a date string into a unix timestamp. | Integer |

---

## Machine
Name: **IshOS**

| Command | Arguments | Description | Returns |
|---|---|---|---|
| `os_hostname` | None | Gets the machine hostname. | String |
| `os_os` | None | Gets the operating system name. | String |
| `os_arch` | None | Gets the CPU architecture. | String |
| `os_getenvvars` | None | Lists all environment variables. | Array of Strings |
| `os_getenvvar`| `<name>` | Gets the value of an environment variable. | String |
| `os_setenvvar`| `<name> <value>` | Sets an environment variable. | Exit Code |
| `os_platform` | None | Returns the machine platform. | String |
| `os_version` | None | Returns the machine version. | String |
| `os_exit` | `[code]` | Exits the shell process immediately. | Exit Code |
| `os_sleep` | `<ms>` | Pauses execution for milliseconds. | Exit Code |
| `os_clear` | None | Clears the terminal output. | Exit Code |
| `os_users` | None | Returns a list of users on the machine. | Array of Strings |

---

## Usage

Because these are built natively into the interpreter, you simply call them like any command and capture their output using variable assignment. The returned data structures natively map to `IshValue` types (such as `Array`, `Map`, `Int`).

```bash
# Capture native strings and ints seamlessly
let host = "$(os_hostname)"
let len = "$(str_len $host)"
out "Host $host is $len characters long"
```

## User Libraries

You can load your own `.ish` scripts into the current environment using the `with` keyword. This merges their function definitions (`func`) into the global scope.

```bash
# Inside sample_lib.ish
func sample_hello() {
    out "Hello from the library!"
}

func sample_returner(val) {
    return $val
}
```

```bash
# Inside main.ish
with sample_lib.ish

func main() {
    sample_hello
    let x = "$(sample_returner 22)"
    out "The value is $x"
}
```
