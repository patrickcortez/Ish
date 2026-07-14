use crate::error::IshError;
use crate::core::ast::IshValue;
use crate::core::io::{StdinStream, StdoutStream, StderrStream, InputStream, OutputStream};
use crate::core::io::color::{Color, ColorManager};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

pub trait StdlibProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn handles_method(&self, method: &str) -> bool;
    fn execute_method(&self, method: &str, args: &[IshValue], gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError>;
}

fn as_string(v: &IshValue) -> String {
    match v {
        IshValue::String(s) => s.clone(),
        IshValue::Int(i) => i.to_string(),
        IshValue::Float(f) => f.to_string(),
        IshValue::Bool(b) => b.to_string(),
        IshValue::Null => "null".to_string(),
        IshValue::Reference(id) => format!("<Reference {}>", id),
    }
}

pub struct IshCommandLine;
static COLOR_MANAGER: OnceLock<Mutex<ColorManager>> = OnceLock::new();

fn get_color_manager() -> &'static Mutex<ColorManager> {
    COLOR_MANAGER.get_or_init(|| Mutex::new(ColorManager::new()))
}

impl StdlibProvider for IshCommandLine {
    fn name(&self) -> &'static str { "CommandLine" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], _gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "OutputLine" => {
                let mut stdout = StdoutStream::new();
                if args.is_empty() {
                    stdout.write_line("").map_err(|e| IshError::ExecutionError(e.to_string()))?;
                } else {
                    stdout.write_line(&as_string(&args[0])).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                }
                Ok(IshValue::Null)
            }
            "Output" => {
                if !args.is_empty() {
                    let mut stdout = StdoutStream::new();
                    stdout.write(&as_string(&args[0])).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                }
                Ok(IshValue::Null)
            }
            "Input" => {
                let mut stdin = StdinStream::new();
                let line = stdin.read_line().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::String(line))
            }
            "Read" => {
                let mut stdin = StdinStream::new();
                let key = stdin.read_key().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::String(key.to_string()))
            }
            "Out" => Ok(IshValue::String("<StdoutStream>".to_string())),
            "Error" => Ok(IshValue::String("<StderrStream>".to_string())),
            "In" => Ok(IshValue::String("<StdinStream>".to_string())),
            "ForeColor" => {
                if args.is_empty() {
                    return Err(IshError::ExecutionError("ForeColor requires a color argument".into()));
                }
                let color_str = as_string(&args[0]);
                let color = if color_str.starts_with('#') || color_str.len() == 6 {
                    Color::from_hex(&color_str).map_err(|e| IshError::ExecutionError(e.to_string()))?
                } else {
                    match color_str.to_lowercase().as_str() {
                        "black" => Color::Black,
                        "red" => Color::Red,
                        "green" => Color::Green,
                        "yellow" => Color::Yellow,
                        "blue" => Color::Blue,
                        "magenta" => Color::Magenta,
                        "cyan" => Color::Cyan,
                        "white" => Color::White,
                        _ => return Err(IshError::ExecutionError(format!("Unknown color: {}", color_str))),
                    }
                };
                let mut cm = get_color_manager().lock().unwrap();
                cm.set_foreground(color).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::Null)
            }
            "BackColor" => {
                if args.is_empty() {
                    return Err(IshError::ExecutionError("BackColor requires a color argument".into()));
                }
                let color_str = as_string(&args[0]);
                let color = if color_str.starts_with('#') || color_str.len() == 6 {
                    Color::from_hex(&color_str).map_err(|e| IshError::ExecutionError(e.to_string()))?
                } else {
                    match color_str.to_lowercase().as_str() {
                        "black" => Color::Black,
                        "red" => Color::Red,
                        "green" => Color::Green,
                        "yellow" => Color::Yellow,
                        "blue" => Color::Blue,
                        "magenta" => Color::Magenta,
                        "cyan" => Color::Cyan,
                        "white" => Color::White,
                        _ => return Err(IshError::ExecutionError(format!("Unknown color: {}", color_str))),
                    }
                };
                let mut cm = get_color_manager().lock().unwrap();
                cm.set_background(color).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::Null)
            }
            "ResetColor" => {
                let mut cm = get_color_manager().lock().unwrap();
                cm.reset().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::Null)
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in CommandLine", method)))
        }
    }
}

pub struct IshStr;

impl StdlibProvider for IshStr {
    fn name(&self) -> &'static str { "Str" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "ToLower" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.ToLower requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).to_lowercase()))
            }
            "ToUpper" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.ToUpper requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).to_uppercase()))
            }
            "Reverse" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.Reverse requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).chars().rev().collect()))
            }
            "Trim" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.Trim requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).trim().to_string()))
            }
            "TrimStart" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.TrimStart requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).trim_start().to_string()))
            }
            "TrimEnd" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.TrimEnd requires 1 argument".into())); }
                Ok(IshValue::String(as_string(&args[0]).trim_end().to_string()))
            }
            "Length" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Str.Length requires 1 argument".into())); }
                Ok(IshValue::Int(as_string(&args[0]).len() as i32))
            }
            "Contains" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Str.Contains requires 2 args".into())); }
                Ok(IshValue::Bool(as_string(&args[0]).contains(&as_string(&args[1]))))
            }
            "IndexOf" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Str.IndexOf requires 2 args".into())); }
                match as_string(&args[0]).find(&as_string(&args[1])) {
                    Some(idx) => Ok(IshValue::Int(idx as i32)),
                    None => Ok(IshValue::Int(-1))
                }
            }
            "Substring" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Str.Substring requires >= 2 args".into())); }
                let s = as_string(&args[0]);
                let start = match &args[1] {
                    IshValue::Int(i) => *i as usize,
                    _ => return Err(IshError::ExecutionError("Substring start must be int".into()))
                };
                let mut end = s.len();
                if args.len() > 2 {
                    if let IshValue::Int(i) = &args[2] { end = *i as usize; }
                }
                if start > s.len() || end > s.len() || start > end {
                    return Err(IshError::ExecutionError("Invalid substring indices".into()));
                }
                Ok(IshValue::String(s[start..end].to_string()))
            }
            "Join" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Str.Join requires array and sep".into())); }
                let sep = as_string(&args[1]);
                if let IshValue::Reference(id) = &args[0] {
                    if let Some(crate::core::gobbler::HeapObject::Array(arr)) = gobbler.get(*id) {
                        let strs: Vec<String> = arr.iter().map(|v| as_string(v)).collect();
                        return Ok(IshValue::String(strs.join(&sep)));
                    }
                }
                Err(IshError::ExecutionError("First argument to Join must be an array reference".into()))
            }
            "Split" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Str.Split requires string and sep".into())); }
                let s = as_string(&args[0]);
                let sep = as_string(&args[1]);
                let parts: Vec<String> = s.split(&sep).map(|x| x.to_string()).collect();
                let ishs: Vec<IshValue> = parts.into_iter().map(IshValue::String).collect();
                let ref_id = gobbler.allocate(crate::core::gobbler::HeapObject::Array(ishs));
                Ok(IshValue::Reference(ref_id))
            }
            "Replace" => {
                if args.len() < 3 { return Err(IshError::ExecutionError("Str.Replace requires 3 args".into())); }
                Ok(IshValue::String(as_string(&args[0]).replace(&as_string(&args[1]), &as_string(&args[2]))))
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in Str", method)))
        }
    }
}

pub struct IshFS;
impl StdlibProvider for IshFS {
    fn name(&self) -> &'static str { "FS" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "Exists" => {
                if args.is_empty() { return Err(IshError::ExecutionError("FS.Exists requires 1 arg".into())); }
                Ok(IshValue::Bool(std::path::Path::new(&as_string(&args[0])).exists()))
            }
            "ReadAllLines" => {
                if args.is_empty() { return Err(IshError::ExecutionError("FS.ReadAllLines requires 1 arg".into())); }
                let content = std::fs::read_to_string(&as_string(&args[0])).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                let lines: Vec<IshValue> = content.lines().map(|l| IshValue::String(l.to_string())).collect();
                let ref_id = gobbler.allocate(crate::core::gobbler::HeapObject::Array(lines));
                Ok(IshValue::Reference(ref_id))
            }
            "ReadAllText" => {
                if args.is_empty() { return Err(IshError::ExecutionError("FS.ReadAllText requires 1 arg".into())); }
                let content = std::fs::read_to_string(&as_string(&args[0])).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::String(content))
            }
            "WriteAllText" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("FS.WriteAllText requires <path> <data> [append]".into())); }
                let append = if args.len() > 2 { matches!(&args[2], IshValue::Bool(true)) } else { false };
                let mut file = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&as_string(&args[0]))
                    .map_err(|e| IshError::ExecutionError(e.to_string()))?;
                file.write_all(as_string(&args[1]).as_bytes()).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(IshValue::Bool(true))
            }
            "List" => {
                if args.is_empty() { return Err(IshError::ExecutionError("FS.List requires 1 arg".into())); }
                let mut entries = Vec::new();
                if let Ok(rd) = std::fs::read_dir(&as_string(&args[0])) {
                    for entry in rd.flatten() {
                        entries.push(IshValue::String(entry.path().display().to_string()));
                    }
                }
                let ref_id = gobbler.allocate(crate::core::gobbler::HeapObject::Array(entries));
                Ok(IshValue::Reference(ref_id))
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in FS", method)))
        }
    }
}

pub struct IshMath;
impl StdlibProvider for IshMath {
    fn name(&self) -> &'static str { "Math" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], _gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        let get_f = |v: &IshValue| -> f32 {
            match v {
                IshValue::Float(f) => *f,
                IshValue::Int(i) => *i as f32,
                IshValue::String(s) => s.parse().unwrap_or(0.0),
                _ => 0.0
            }
        };
        match method {
            "Abs" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Math.Abs requires 1 arg".into())); }
                Ok(IshValue::Float(get_f(&args[0]).abs()))
            }
            "Ceiling" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Math.Ceiling requires 1 arg".into())); }
                Ok(IshValue::Float(get_f(&args[0]).ceil()))
            }
            "Floor" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Math.Floor requires 1 arg".into())); }
                Ok(IshValue::Float(get_f(&args[0]).floor()))
            }
            "Round" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Math.Round requires 1 arg".into())); }
                Ok(IshValue::Float(get_f(&args[0]).round()))
            }
            "Pow" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Math.Pow requires 2 args".into())); }
                Ok(IshValue::Float(get_f(&args[0]).powf(get_f(&args[1]))))
            }
            "Min" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Math.Min requires 2 args".into())); }
                Ok(IshValue::Float(get_f(&args[0]).min(get_f(&args[1]))))
            }
            "Max" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("Math.Max requires 2 args".into())); }
                Ok(IshValue::Float(get_f(&args[0]).max(get_f(&args[1]))))
            }
            "Sqrt" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Math.Sqrt requires 1 arg".into())); }
                Ok(IshValue::Float(get_f(&args[0]).sqrt()))
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in Math", method)))
        }
    }
}

pub struct IshTime;
impl StdlibProvider for IshTime {
    fn name(&self) -> &'static str { "Time" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], _gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "Now" => {
                let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                Ok(IshValue::Int(s as i32))
            }
            "Unix" => {
                let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                Ok(IshValue::Int(s as i32))
            }
            "Format" => {
                if args.is_empty() { return Err(IshError::ExecutionError("Time.Format requires 1 arg".into())); }
                Ok(IshValue::String(format!("Formatted: {}", as_string(&args[0]))))
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in Time", method)))
        }
    }
}

pub struct IshNet;
impl StdlibProvider for IshNet {
    fn name(&self) -> &'static str { "Net" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, _args: &[IshValue], _gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "Get" => Ok(IshValue::String("Net.Get not implemented natively yet".into())),
            "Post" => Ok(IshValue::String("Net.Post not implemented natively yet".into())),
            "Download" => Ok(IshValue::Bool(false)),
            _ => Err(IshError::ExecutionError(format!("Method {} not found in Net", method)))
        }
    }
}

pub struct IshOS;
impl StdlibProvider for IshOS {
    fn name(&self) -> &'static str { "OS" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], _gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "GetEnv" => {
                if args.is_empty() { return Err(IshError::ExecutionError("OS.GetEnv requires 1 arg".into())); }
                match std::env::var(as_string(&args[0])) {
                    Ok(v) => Ok(IshValue::String(v)),
                    Err(_) => Ok(IshValue::Null)
                }
            }
            "SetEnv" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("OS.SetEnv requires 2 args".into())); }
                unsafe { std::env::set_var(as_string(&args[0]), as_string(&args[1])); }
                Ok(IshValue::Bool(true))
            }
            "Platform" => Ok(IshValue::String(std::env::consts::OS.to_string())),
            "Arch" => Ok(IshValue::String(std::env::consts::ARCH.to_string())),
            "Cwd" => Ok(IshValue::String(std::env::current_dir().unwrap_or_default().display().to_string())),
            _ => Err(IshError::ExecutionError(format!("Method {} not found in OS", method)))
        }
    }
}

pub struct IshExtProc;
impl StdlibProvider for IshExtProc {
    fn name(&self) -> &'static str { "ExtProc" }
    fn handles_method(&self, _method: &str) -> bool { true }
    fn execute_method(&self, method: &str, args: &[IshValue], gobbler: &mut crate::core::gobbler::Gobbler) -> Result<IshValue, IshError> {
        match method {
            "Start" => {
                if args.is_empty() { return Err(IshError::ExecutionError("ExtProc.Start requires <program> [args[]]".into())); }
                let program = as_string(&args[0]);
                let mut cmd = std::process::Command::new(program);
                if args.len() > 1 {
                    if let IshValue::Reference(id) = &args[1] {
                        if let Some(crate::core::gobbler::HeapObject::Array(arr)) = gobbler.get(*id) {
                            for a in arr {
                                cmd.arg(as_string(a));
                            }
                        }
                    } else if let IshValue::String(s) = &args[1] {
                        cmd.arg(s); // Fallback if single string passed
                    }
                }
                
                let output = cmd.output().map_err(|e| IshError::ExecutionError(format!("ExtProc.Start failed: {}", e)))?;
                let mut map = std::collections::HashMap::new();
                map.insert("ExitCode".to_string(), IshValue::Int(output.status.code().unwrap_or(-1) as i32));
                map.insert("StandardOutput".to_string(), IshValue::String(String::from_utf8_lossy(&output.stdout).to_string()));
                map.insert("StandardError".to_string(), IshValue::String(String::from_utf8_lossy(&output.stderr).to_string()));
                
                let ref_id = gobbler.allocate(crate::core::gobbler::HeapObject::Object {
                    class_name: "ExtProcResult".to_string(),
                    properties: map,
                });
                
                Ok(IshValue::Reference(ref_id))
            }
            _ => Err(IshError::ExecutionError(format!("Method {} not found in ExtProc", method)))
        }
    }
}
