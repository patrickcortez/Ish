use crate::error::IshError;

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
