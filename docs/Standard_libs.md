# Standard Libraries

The Standard libraries of Ish are as follows:

## File System I/O

Name: **IshFS**

- API's:

  - *fs_readfile <path>* - reads a file line by line and returns an array of string containing the file content. Bases it off the path given.

  - *fs_writefile <path> <data> [append]* - writes an array or just a single string of data to the file path given and a boolean appends or overwrites it. append is true by default.

  - *fs_createfile <path>* - Creates file on the path, returns an exit code.
  - *fs_deletefile <path>* - Deletes file on the path, returns an exit code.
  - *fs_exists <path>* - Checks if a file or directory exists, returns an exit code.
  - *fs_createdir <path>* - Creates directory on the path, returns an exit code.
  - *fs_deletedir <path>* - Deletes directory on the path, returns an exit code.
  - *fs_list <path>* - Lists the contents of a directory, returns an array of strings.
  - *fs_copy <source> <dest>* - Copies a file or folder to a destination, returns an exit code.
  - *fs_move <source> <dest>* - Moves or renames a file or folder to a destination, returns an exit code.
  - *fs_getfileperm <path>* - returns a string containing the file permissions in octal format.
  - *fs_getdirperm <path>* - returns a string containing the directory permissions in octal format.

## Networking

Name: **IshNet**

- API's:

  - *net_isavailable* - Checks if internet is available. Returns 0 if available, 1 if not.

  - *net_ssid* - Returns a string containing the SSID of the current wifi network.

  - *net_ip* - Returns an array of strings containing the IP addresses of the current wifi network.

  - *net_ping <host>* - Pings a host (domain or IP address) and returns 0 or 1 depending if the host is recheable.

  - *net_get <url> [headers]* - Sends an HTTP GET request to the specified URL. Returns a string containing the response body.

  - *net_post <url> <data> [headers]* - Sends an HTTP POST request to the specified URL with the provided data payload. Returns a string containing the response body.

  - *net_resolve <domain>* - Performs a DNS lookup for the given domain name and returns its IP address as a string.

  - *net_getsecure <url> [headers]* - Identical to `net_get`, but strictly enforces a secure SSL/TLS (HTTPS) connection.

## String Utils

Name: **IshStr**

- API's:

  - *str_tolower <string>* - returns a lowered version of the string given.

  - *str_toupper <string>* - returns a uppered version of the string given.

  - *str_substr <string> <start> [end]* - returns a substring of the string given starting from the index `start` to `end`. If `end` is not given it will return the substring from `start` to the end of the string.

  - *str_join <array> <separator>* - joins an array of strings into a single string with the separator given.

  - *str_split <string> <separator>* - splits a string into an array of strings using the separator given.

  - *str_replace <string> <old> <new>* - replaces all occurrences of `old` with `new` in the string given.

  - *str_contains <string> <substring>* - returns true if the string contains the substring.

  - *str_find <string> <substring>* - returns the index of the first occurrence of the substring in the string.

  - *str_len <string>* - returns the length of the string.

  - *str_reverse <string>* - returns the reversed version of the string given.

  - *str_trim <string>* - returns a trimmed version of the string given.

  - *str_trimstart <string>* - returns a trimmed version of the string given starting from the first non-whitespace character.

  - *str_trimend <string>* - returns a trimmed version of the string given ending at the last non-whitespace character.

## Date and Time

Name: **IshTime**

- API's:

  - *time_now* - returns a string containing the current date and time.

  - *time_unix* - returns a string containing the current unix time.

  - *time_format <timestamp_or_rfc3339> <format>* - returns a string containing the date and time formatted according to the given format. Supports unix timestamps or RFC3339 strings.

  - *time_parse <string> <format>* - returns a string containing the unix timestamp of the parsed date and time from the given string and format.

## Machine

Name: **IshOS**

- API's:

  - *os_hostname* - returns a string containing the hostname of the machine.

  - *os_os* - returns a string containing the operating system of the machine.

  - *os_arch* - returns a string containing the architecture of the machine.

  - *os_getenvvars* - returns a string array of env vars.

  - *os_getenvvar <name>* - returns a string containing the value of the environment variable with the given name.

  - *os_setenvvar <name> <value>* - sets the value of the environment variable with the given name to the given value.

  - *os_platform* - returns the platform of the machine.

  - *os_version* - returns the version of the machine.

  - *os_exit [code]* - exits the script with the given exit code.

  - *os_sleep <ms>* - pauses script execution for the given milliseconds.

  - *os_clear* - clears the terminal screen.

  - *os_users* - returns the list of users on the machine.

## Usage

The Standard Libraries are natively built into the Ish Rust interpreter to ensure true cross-platform compatibility across Windows, macOS, and Linux without needing to manage DLLs or shared libraries.

Because they are built-in, you simply call them like any native command and capture their output using variable assignment.

For example, when a user types:

```ish
declare host = os_hostname
```

The interpreter safely executes the native `os_hostname` command and assigns the string result to the local variable `host`.

## User Libraries

Users can load other `.ish` scripts into their current scripts using the `with` keyword.

When `with` is called, Ish reads the target script and merges its function (`fn`) definitions into the current global scope. This allows you to split large projects into smaller, modular files.

```ish
# Inside sample_lib.ish
fn sample_hello() {
    out "Hello from the library!"
}

fn sample_returner(val) {
    return out $val
}
```

```ish
# Inside main.ish
with sample_lib.ish

fn func() {
    sample_hello
    declare x = sample_returner 22
    out "The value is $x"
}
```
