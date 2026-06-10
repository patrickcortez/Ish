use crate::error::IshError;
use std::io::Write;

/// A trait for providing Standard Library commands to the Executor
pub trait StdlibProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn handles_command(&self, cmd: &str) -> bool;
    fn execute(&self, cmd: &str, args: &[String]) -> Result<String, IshError>;
}

pub struct IshStr;

impl StdlibProvider for IshStr {
    fn name(&self) -> &'static str {
        "IshStr"
    }

    fn handles_command(&self, cmd: &str) -> bool {
        cmd.starts_with("str_")
    }

    fn execute(&self, cmd: &str, args: &[String]) -> Result<String, IshError> {
        match cmd {
            "str_tolower" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_tolower requires 1 argument".to_string())); }
                Ok(args[0].to_lowercase())
            }
            "str_toupper" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_toupper requires 1 argument".to_string())); }
                Ok(args[0].to_uppercase())
            }
            "str_reverse" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_reverse requires 1 argument".to_string())); }
                Ok(args[0].chars().rev().collect())
            }
            "str_trim" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_trim requires 1 argument".to_string())); }
                Ok(args[0].trim().to_string())
            }
            "str_trimstart" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_trimstart requires 1 argument".to_string())); }
                Ok(args[0].trim_start().to_string())
            }
            "str_trimend" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_trimend requires 1 argument".to_string())); }
                Ok(args[0].trim_end().to_string())
            }
            "str_len" => {
                if args.is_empty() { return Err(IshError::ExecutionError("str_len requires 1 argument".to_string())); }
                Ok(args[0].len().to_string())
            }
            "str_contains" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("str_contains requires 2 arguments: <string> <substring>".to_string())); }
                Ok(args[0].contains(&args[1]).to_string())
            }
            "str_find" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("str_find requires 2 arguments: <string> <substring>".to_string())); }
                match args[0].find(&args[1]) {
                    Some(idx) => Ok(idx.to_string()),
                    None => Ok("-1".to_string())
                }
            }
            "str_substr" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("str_substr requires at least 2 arguments: <string> <start> [end]".to_string())); }
                let start = args[1].parse::<usize>().map_err(|_| IshError::ExecutionError("Invalid start index".to_string()))?;
                let mut end = args[0].len();
                if args.len() > 2 {
                    end = args[2].parse::<usize>().map_err(|_| IshError::ExecutionError("Invalid end index".to_string()))?;
                }
                if start > args[0].len() || end > args[0].len() || start > end {
                    return Err(IshError::ExecutionError("Invalid substring indices".to_string()));
                }
                Ok(args[0][start..end].to_string())
            }
            "str_join" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("str_join requires an array and a separator".to_string())); }
                let sep = &args[args.len() - 1];
                let mut arr_str = args[0].clone();
                if arr_str.starts_with('[') && arr_str.ends_with(']') {
                    arr_str = arr_str[1..arr_str.len() - 1].to_string();
                }
                let items: Vec<&str> = arr_str.split(", ").collect();
                Ok(items.join(sep))
            }
            "str_split" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("str_split requires 2 arguments: <string> <separator>".to_string())); }
                let parts: Vec<&str> = args[0].split(&args[1]).collect();
                // We return parts joined by space to be used as array in Ish
                Ok(parts.join(" "))
            }
            "str_replace" => {
                if args.len() < 3 { return Err(IshError::ExecutionError("str_replace requires 3 arguments: <string> <old> <new>".to_string())); }
                Ok(args[0].replace(&args[1], &args[2]))
            }
            _ => Err(IshError::ExecutionError(format!("Command {} not implemented in IshStr", cmd)))
        }
    }
}

pub struct IshFS;

impl StdlibProvider for IshFS {
    fn name(&self) -> &'static str {
        "IshFS"
    }

    fn handles_command(&self, cmd: &str) -> bool {
        cmd.starts_with("fs_")
    }

    fn execute(&self, cmd: &str, args: &[String]) -> Result<String, IshError> {
        match cmd {
            "fs_exists" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_exists requires 1 argument".to_string())); }
                Ok(std::path::Path::new(&args[0]).exists().to_string())
            }
            "fs_readfile" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_readfile requires 1 argument".to_string())); }
                let content = std::fs::read_to_string(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                // Return as an Ish Array representation of lines
                let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
                Ok(format!("[{}]", lines.join(", ")))
            }
            "fs_writefile" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("fs_writefile requires <path> <data> [append]".to_string())); }
                let mut append = true;
                if args.len() > 2 {
                    append = args[2] == "true";
                }
                
                let mut data_str = args[1].clone();
                if data_str.starts_with('[') && data_str.ends_with(']') {
                    data_str = data_str[1..data_str.len() - 1].to_string();
                }
                
                let mut file = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&args[0])
                    .map_err(|e| IshError::ExecutionError(e.to_string()))?;
                file.write_all(data_str.as_bytes()).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_createfile" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_createfile requires 1 argument".to_string())); }
                std::fs::File::create(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_deletefile" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_deletefile requires 1 argument".to_string())); }
                std::fs::remove_file(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_createdir" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_createdir requires 1 argument".to_string())); }
                std::fs::create_dir_all(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_deletedir" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_deletedir requires 1 argument".to_string())); }
                std::fs::remove_dir_all(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_list" => {
                if args.is_empty() { return Err(IshError::ExecutionError("fs_list requires 1 argument".to_string())); }
                let mut entries = Vec::new();
                for entry in std::fs::read_dir(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))? {
                    let entry = entry.map_err(|e| IshError::ExecutionError(e.to_string()))?;
                    entries.push(entry.file_name().to_string_lossy().to_string());
                }
                Ok(format!("[{}]", entries.join(", ")))
            }
            "fs_copy" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("fs_copy requires <source> <dest>".to_string())); }
                std::fs::copy(&args[0], &args[1]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_move" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("fs_move requires <source> <dest>".to_string())); }
                std::fs::rename(&args[0], &args[1]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok("".to_string())
            }
            "fs_getfileperm" | "fs_getdirperm" => {
                if args.is_empty() { return Err(IshError::ExecutionError("requires 1 argument".to_string())); }
                let meta = std::fs::metadata(&args[0]).map_err(|e| IshError::ExecutionError(e.to_string()))?;
                let perms = meta.permissions();
                if perms.readonly() {
                    Ok("0444".to_string())
                } else {
                    Ok("0644".to_string()) // Simulating default rw-r--r--
                }
            }
            _ => Err(IshError::ExecutionError(format!("Command {} not implemented in IshFS", cmd)))
        }
    }
}

pub struct IshTime;

impl StdlibProvider for IshTime {
    fn name(&self) -> &'static str {
        "IshTime"
    }

    fn handles_command(&self, cmd: &str) -> bool {
        cmd.starts_with("time_")
    }

    fn execute(&self, cmd: &str, args: &[String]) -> Result<String, IshError> {
        match cmd {
            "time_now" => {
                Ok(chrono::Utc::now().to_rfc3339())
            }
            "time_unix" => {
                Ok(chrono::Utc::now().timestamp().to_string())
            }
            "time_format" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("time_format requires <timestamp_or_rfc3339> <format>".to_string())); }
                let dt = if let Ok(unix) = args[0].parse::<i64>() {
                    chrono::DateTime::from_timestamp(unix, 0).ok_or_else(|| IshError::ExecutionError("Invalid unix timestamp".to_string()))?
                } else {
                    chrono::DateTime::parse_from_rfc3339(&args[0])
                        .map_err(|e| IshError::ExecutionError(e.to_string()))?
                        .with_timezone(&chrono::Utc)
                };
                Ok(dt.format(&args[1]).to_string())
            }
            "time_parse" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("time_parse requires <time_string> <format>".to_string())); }
                let dt = chrono::NaiveDateTime::parse_from_str(&args[0], &args[1])
                    .map_err(|e| IshError::ExecutionError(e.to_string()))?;
                Ok(dt.and_utc().timestamp().to_string())
            }
            _ => Err(IshError::ExecutionError(format!("Command {} not implemented in IshTime", cmd)))
        }
    }
}

pub struct IshNet;

impl StdlibProvider for IshNet {
    fn name(&self) -> &'static str {
        "IshNet"
    }

    fn handles_command(&self, cmd: &str) -> bool {
        cmd.starts_with("net_")
    }

    fn execute(&self, cmd: &str, args: &[String]) -> Result<String, IshError> {
        match cmd {
            "net_isavailable" => {
                use std::net::ToSocketAddrs;
                if "1.1.1.1:53".to_socket_addrs().is_ok() || "8.8.8.8:53".to_socket_addrs().is_ok() {
                    Ok("true".to_string())
                } else {
                    Ok("false".to_string())
                }
            }
            "net_ssid" => {
                if cfg!(target_os = "windows") {
                    if let Ok(output) = std::process::Command::new("netsh").args(&["wlan", "show", "interfaces"]).output() {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            if line.trim_start().starts_with("SSID") {
                                let parts: Vec<&str> = line.split(':').collect();
                                if parts.len() >= 2 {
                                    return Ok(parts[1].trim().to_string());
                                }
                            }
                        }
                    }
                }
                Ok("".to_string())
            }
            "net_ip" => {
                if let Ok(output) = std::process::Command::new("curl").args(&["-s", "https://api.ipify.org"]).output() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(IshError::ExecutionError("Failed to fetch IP".to_string()))
                }
            }
            "net_ping" => {
                if args.is_empty() { return Err(IshError::ExecutionError("net_ping requires 1 argument".to_string())); }
                let host = &args[0];
                let (prog, ping_args) = if cfg!(target_os = "windows") {
                    ("ping", vec!["-n", "1", host])
                } else {
                    ("ping", vec!["-c", "1", host])
                };
                if let Ok(status) = std::process::Command::new(prog).args(&ping_args).status() {
                    if status.success() {
                        return Ok("true".to_string());
                    }
                }
                Ok("false".to_string())
            }
            "net_resolve" => {
                if args.is_empty() { return Err(IshError::ExecutionError("net_resolve requires 1 argument".to_string())); }
                use std::net::ToSocketAddrs;
                let query = if args[0].contains(':') { args[0].clone() } else { format!("{}:80", args[0]) };
                match query.to_socket_addrs() {
                    Ok(mut addrs) => {
                        if let Some(addr) = addrs.next() {
                            Ok(addr.ip().to_string())
                        } else {
                            Err(IshError::ExecutionError("Could not resolve host".to_string()))
                        }
                    }
                    Err(e) => Err(IshError::ExecutionError(e.to_string()))
                }
            }
            "net_get" | "net_getsecure" => {
                if args.is_empty() { return Err(IshError::ExecutionError(format!("{} requires 1 argument", cmd))); }
                if let Ok(output) = std::process::Command::new("curl").args(&["-sL", &args[0]]).output() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(IshError::ExecutionError("Failed to run curl".to_string()))
                }
            }
            "net_post" => {
                if args.len() < 2 { return Err(IshError::ExecutionError("net_post requires <url> <data>".to_string())); }
                if let Ok(output) = std::process::Command::new("curl").args(&["-sL", "-X", "POST", "-d", &args[1], &args[0]]).output() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Err(IshError::ExecutionError("Failed to run curl".to_string()))
                }
            }
            _ => Err(IshError::ExecutionError(format!("Command {} not implemented in IshNet", cmd)))
        }
    }
}
