use std::env;
use std::fs;
use std::path::Path;
use std::process;
use std::io::Write;

use crate::core::ast::IshValue;
use std::collections::HashMap;

/// Tries to execute an internal command.
/// Returns Ok(Some(IshValue)) if it was an internal command and it succeeded.
/// Returns Ok(None) if it is NOT an internal command.
/// Returns Err(error_message) if it was an internal command but it failed.
pub fn execute_internal(program: &str, args: &[String], gobbler: &mut crate::core::gobbler::Gobbler) -> Result<Option<IshValue>, String> {
    match program {
        "change" => {
            let path = if args.is_empty() {
                env::var("USERPROFILE").or_else(|_| env::var("HOME")).unwrap_or_else(|_| String::from("."))
            } else {
                args[0].clone()
            };
            if let Err(e) = env::set_current_dir(&path) {
                return Err(format!("change error: {}", e));
            }
            Ok(Some(IshValue::String(String::new())))
        }
        "quit" => {
            let code = if !args.is_empty() {
                args[0].parse::<i32>().unwrap_or(0)
            } else {
                0
            };
            process::exit(code);
        }
        "declare" => {
            if args.is_empty() {
                let mut out = String::new();
                for (key, val) in env::vars() {
                    out.push_str(&format!("{}={}\n", key, val));
                }
                return Ok(Some(IshValue::String(out)));
            }
            let mut i = 0;
            while i < args.len() {
                if i + 2 < args.len() && args[i + 1] == "=" {
                    unsafe { env::set_var(&args[i], &args[i + 2]); }
                    i += 3;
                } else if i + 1 < args.len() && args[i].ends_with('=') {
                    unsafe { env::set_var(args[i].trim_end_matches('='), &args[i + 1]); }
                    i += 2;
                } else if i + 1 < args.len() && args[i + 1].starts_with('=') {
                    unsafe { env::set_var(&args[i], args[i + 1].trim_start_matches('=')); }
                    i += 2;
                } else {
                    let parts: Vec<&str> = args[i].splitn(2, '=').collect();
                    if parts.len() == 2 {
                        unsafe { env::set_var(parts[0], parts[1]); }
                    } else {
                        unsafe { env::set_var(parts[0], ""); }
                    }
                    i += 1;
                }
            }
            Ok(Some(IshValue::String(String::new())))
        }
        "out" => {
            Ok(Some(IshValue::String(args.join(" ") + "\n")))
        }
        "cwd" => {
            match env::current_dir() {
                Ok(dir) => Ok(Some(IshValue::String(format!("{}\n", dir.display())))),
                Err(e) => Err(format!("cwd error: {}", e)),
            }
        }
        "show" => {
            let dir = if args.is_empty() { "." } else { &args[0] };
            let mut entries_arr = Vec::new();
            match fs::read_dir(dir) {
                Ok(entries) => {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            let mut map = HashMap::new();
                            if let Ok(file_name) = entry.file_name().into_string() {
                                map.insert("name".to_string(), IshValue::String(file_name));
                            }
                            if let Ok(metadata) = entry.metadata() {
                                map.insert("size".to_string(), IshValue::Int(metadata.len() as i32));
                                map.insert("is_dir".to_string(), IshValue::Bool(metadata.is_dir()));
                                map.insert("is_file".to_string(), IshValue::Bool(metadata.is_file()));
                            }
                            let map_ref = gobbler.allocate(crate::core::gobbler::HeapObject::Map(map));
                            entries_arr.push(IshValue::Reference(map_ref));
                        }
                    }
                    let arr_ref = gobbler.allocate(crate::core::gobbler::HeapObject::Array(entries_arr));
                    Ok(Some(IshValue::Reference(arr_ref)))
                }
                Err(e) => Err(format!("show error: {}", e)),
            }
        }
        "read" => {
            if args.is_empty() {
                return Err("read error: expected file path".to_string());
            }
            let mut out = String::new();
            for file in args {
                match fs::read_to_string(file) {
                    Ok(contents) => {
                        out.push_str(&contents);
                        if !contents.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    Err(e) => return Err(format!("read error: {}", e)),
                }
            }
            Ok(Some(IshValue::String(out)))
        }
        "create" => {
            if args.is_empty() {
                return Err("create error: expected path".to_string());
            }
            let mut is_dir = false;
            let mut paths = Vec::new();
            for arg in args {
                if arg == "-d" {
                    is_dir = true;
                } else if arg == "-f" {
                    is_dir = false;
                } else {
                    paths.push(arg.clone());
                }
            }
            if paths.is_empty() {
                return Err("create error: expected path after flags".to_string());
            }
            for path in paths {
                if is_dir {
                    if let Err(e) = fs::create_dir_all(&path) {
                        return Err(format!("create dir error on {}: {}", path, e));
                    }
                } else {
                    if let Err(e) = fs::File::create(&path) {
                        return Err(format!("create file error on {}: {}", path, e));
                    }
                }
            }
            Ok(Some(IshValue::String(String::new())))
        }
        "input" => {
            if !args.is_empty() {
                eprint!("{} ", args.join(" "));
                let _ = std::io::stderr().flush();
            }
            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_err() {
                return Err("Failed to read input".to_string());
            }
            Ok(Some(IshValue::String(input.trim_end_matches('\n').trim_end_matches('\r').to_string())))
        }
        "inputkey" => {
            use crossterm::event::{read, Event, KeyCode};
            use crossterm::terminal::{enable_raw_mode, disable_raw_mode};
            if !args.is_empty() {
                eprint!("{} ", args.join(" "));
                let _ = std::io::stderr().flush();
            }
            let _ = enable_raw_mode();
            let result = match read() {
                Ok(Event::Key(key_event)) => {
                    let _ = std::io::stdout().flush();
                    match key_event.code {
                        KeyCode::Char(c) => Ok(Some(IshValue::String(c.to_string()))),
                        KeyCode::Enter => Ok(Some(IshValue::String("\n".to_string()))),
                        _ => Ok(Some(IshValue::String(String::from(" ")))),
                    }
                }
                _ => Ok(Some(IshValue::String(String::from(" ")))),
            };
            let _ = disable_raw_mode();
            result
        }
        "irm" => {
            if args.is_empty() {
                return Err("irm error: expected path".to_string());
            }
            let mut recursive = false;
            let mut paths = Vec::new();
            for arg in args {
                if arg == "-r" || arg == "-R" || arg == "--recursive" {
                    recursive = true;
                } else {
                    paths.push(arg.clone());
                }
            }
            for path in paths {
                let p = Path::new(&path);
                if p.is_dir() {
                    if recursive {
                        if let Err(e) = fs::remove_dir_all(p) {
                            return Err(format!("irm error on {}: {}", path, e));
                        }
                    } else {
                        if let Err(e) = fs::remove_dir(p) {
                            return Err(format!("irm error on {}: {}", path, e));
                        }
                    }
                } else {
                    if let Err(e) = fs::remove_file(p) {
                        return Err(format!("irm error on {}: {}", path, e));
                    }
                }
            }
            Ok(Some(IshValue::String(String::new())))
        }
        "expr" => {
            if args.is_empty() {
                return Err("expr error: missing expression".to_string());
            }
            let expr_str = args.join(" ");
            match eval_math(&expr_str) {
                Ok(val) => Ok(Some(IshValue::String(format!("{}\n", val)))),
                Err(e) => Err(format!("expr error: {}", e)),
            }
        }
        "help" => {
            let help_text = "\
Ish Built-in Commands:
  help                    - Show this help message
  change <dir>            - Change the current directory (cd)
  quit [code]             - Exit the shell (exit)
  declare [var=val]       - Set or display environment variables (export)
  out <text>              - Print text to standard output (echo)
  cwd                     - Print current working directory (pwd)
  show [dir]              - List directory contents (ls)
  read <file>             - Read and output file contents (cat)
  create <-d|-f> <name>   - Create a directory (-d) or file (-f) (mkdir/touch)
  irm [-r] <path>         - Remove file or directory (rm)
  input [prompt]          - Read a line of input into $INPUT variable
  inputkey [prompt]       - Read a single keypress into $INPUT variable
  expr <expression>       - Evaluate a mathematical expression
  jobs                    - List all background jobs
  fg <job_id>             - Bring a background job to the foreground
  kill <job_id>           - Kill a background job
  grep <pattern> <file>   - Search file for pattern (intercepted via OS)
  curl/wget <url>         - Fetch URL contents (intercepted via OS)
  find <pattern>          - Find files (intercepted via OS)
  clear/cls               - Clear the screen (intercepted via OS)
";
            Ok(Some(crate::core::ast::IshValue::String(help_text.to_string())))
        }
        _ => Ok(None),
    }
}

pub fn eval_math(expr: &str) -> Result<f64, String> {
    let mut tokens = Vec::new();
    let mut current_num = String::new();
    
    for c in expr.chars() {
        if c.is_whitespace() { continue; }
        if c == '+' || c == '-' || c == '*' || c == '/' || c == '(' || c == ')' {
            if !current_num.is_empty() {
                tokens.push(current_num.clone());
                current_num.clear();
            }
            tokens.push(c.to_string());
        } else {
            current_num.push(c);
        }
    }
    if !current_num.is_empty() {
        tokens.push(current_num);
    }

    let mut pos = 0;
    let result = parse_expr(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!("Unexpected token at end: {}", tokens[pos]));
    }
    Ok(result)
}

fn parse_expr(tokens: &[String], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        let op = &tokens[*pos];
        if op == "+" || op == "-" {
            *pos += 1;
            let right = parse_term(tokens, pos)?;
            if op == "+" { left += right; } else { left -= right; }
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_term(tokens: &[String], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_factor(tokens, pos)?;
    while *pos < tokens.len() {
        let op = &tokens[*pos];
        if op == "*" || op == "/" {
            *pos += 1;
            let right = parse_factor(tokens, pos)?;
            if op == "*" { left *= right; } else { left /= right; }
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_factor(tokens: &[String], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() { return Err("Unexpected end of expression".to_string()); }
    let token = &tokens[*pos];
    *pos += 1;
    if token == "(" {
        let val = parse_expr(tokens, pos)?;
        if *pos >= tokens.len() || tokens[*pos] != ")" {
            return Err("Missing closing parenthesis".to_string());
        }
        *pos += 1; // consume ")"
        Ok(val)
    } else {
        token.parse::<f64>().map_err(|_| format!("Invalid number: {}", token))
    }
}
