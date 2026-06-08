# Ish Shell Guide

Welcome to the **Ish (Intelli-Shell)** User Guide! Ish is a cross-platform system shell written in Rust using Ratatui. It is designed to be an aesthetically pleasing and highly efficient interface for both casual users and power users.

## The TUI Layout
Ish completely reimagines the terminal interface by using fixed, structured boxes instead of a traditional scrolling prompt.
- **Input Box**: Anchored firmly at the bottom of the screen. You type all your commands here. Features an interactive blinking caret and allows for mid-command editing using the `Left` and `Right` arrow keys.
- **Output Box**: Displays all standard output and standard error from your commands.
  - *Scrolling*: Navigate up and down through massive output logs using `PageUp`/`PageDown` or `Ctrl+Up`/`Ctrl+Down`.
- **Suggestions Box**: A dynamic overlay that hovers right above the Input Box. It provides intelligent, real-time autocomplete suggestions based on your history, the local filesystem, and native OS executables. 
  - *Scrolling*: Navigate suggestions using `Up` and `Down`. 
  - *Accepting*: Press `Right Arrow` or `Tab` to insert the currently highlighted suggestion into your input box.

## Built-In Commands
Ish handles normal OS commands natively, but also provides internal built-ins prefixed by a colon `:`.

- `:Color <Target> <Color>`
  Changes the visual color scheme of Ish's UI components.
  - **Targets**: `--inputbox`, `--output`, `--banner`
  - **Colors**: Valid color names (e.g., `Red`, `Green`, `Cyan`) or Hexadecimal values.
  
- `:Toggle <Flag>`
  Toggles internal shell features.
  - **Flags**: `--autocd true/false` (Enables changing directories automatically by typing a path), `--suggestions true/false` (Toggles the suggestion box visibility).

- `:Editor <editor_name>`
  Sets the default text editor for automatically opening files when navigating the terminal.

- `cd <path>`
  Changes the current working directory.

- `exit` or `quit`
  Gracefully terminates the shell and flushes your history to disk.

## Native PowerShell Integration (Windows)
When running Ish on Windows, the shell seamlessly bridges into the OS.
- **Object Piping**: When you execute a pipeline consisting entirely of native Windows/PowerShell commands (e.g. `Get-ChildItem : Where-Object Name -eq "test"`), Ish optimizes the execution by aggregating it into a unified native PowerShell pipeline. This means `.NET` objects are fully preserved between commands, exactly like native PowerShell!
- **Cmdlet Autocomplete**: Ish automatically parses and caches all native PowerShell Cmdlets, Functions, and Aliases in the background when it boots. This means you will immediately get autocomplete suggestions for standard Windows commands like `Invoke-WebRequest` without any configuration.
