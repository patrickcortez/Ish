use crate::core::ast::{AstNode, AstNodeKind, IshValue};
use crate::error::IshError;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;
use crate::core::stdlib::{StdlibProvider, IshStr, IshFS, IshTime, IshNet, IshOS};
use crate::core::registry::Registry;
use crate::managers::job_controller::JobController;

pub struct Executor {
    pub script_args: Vec<String>,
    pub last_exit_code: i32,
    pub variables: Vec<HashMap<String, IshValue>>,
    pub function_bases: Vec<usize>,
    pub return_value: Option<IshValue>,
    pub functions: HashMap<String, AstNode>,
    pub returning: bool,
    pub breaking: bool,
    pub continuing: bool,
    pub stdlib_providers: Vec<Box<dyn StdlibProvider>>,
    pub registry: Registry,
    /// Tracks which class context we are currently executing within (for access checks).
    pub current_class: Option<String>,
}

impl Executor {
    pub fn new(script_args: Vec<String>) -> Self {
        Self { 
            script_args, 
            last_exit_code: 0,
            variables: vec![HashMap::new()],
            function_bases: vec![0],
            return_value: None,
            functions: HashMap::new(),
            returning: false,
            breaking: false,
            continuing: false,
            stdlib_providers: vec![Box::new(IshStr), Box::new(IshFS), Box::new(IshTime), Box::new(IshNet), Box::new(IshOS)],
            registry: Registry::new(),
            current_class: None,
        }
    }

    fn resolve_var(&mut self, s: &str, jobs: &mut JobController) -> Result<String, IshError> {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '$' {
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume '('
                    if chars.peek() == Some(&'(') {
                        chars.next(); // consume second '('
                        let mut inner = String::new();
                        let mut depth = 2;
                        while let Some(ic) = chars.next() {
                            if ic == '(' {
                                depth += 1;
                            } else if ic == ')' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                } else if depth == 1 {
                                    continue; // First ')' of '))'
                                }
                            }
                            inner.push(ic);
                        }
                        if depth > 0 {
                            return Err(IshError::ParseError("Unclosed math expansion `$((`".to_string()));
                        }
                        if chars.peek() == Some(&')') {
                            chars.next(); // Consume outer ')'
                        }
                        let resolved_inner = self.resolve_var(&inner, jobs)?;
                        match crate::core::utils::eval_math(&resolved_inner) {
                            Ok(val) => result.push_str(&val.to_string()),
                            Err(e) => return Err(IshError::ExecutionError(format!("Math error: {}", e))),
                        }
                        continue;
                    } else {
                        // Subshell expansion
                        let mut inner = String::new();
                        let mut depth = 1;
                        while let Some(ic) = chars.next() {
                            if ic == '(' {
                                depth += 1;
                            } else if ic == ')' {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            inner.push(ic);
                        }
                        if depth > 0 {
                            return Err(IshError::ParseError("Unclosed subshell `$(`".to_string()));
                        }
                        let mut tokenizer = crate::core::tokenizer::Tokenizer::new(&inner);
                        if let Ok(tokens) = tokenizer.tokenize() {
                            let mut parser = crate::core::parser::Parser::new(tokens);
                            if let Ok(ast) = parser.parse() {
                                let out = self.capture_output(&ast, jobs)?;
                                result.push_str(&out);
                            }
                        }
                        continue;
                    }
                }

                let mut var_name = String::new();
                if let Some(&nc) = chars.peek() {
                    if "?!@#$0".contains(nc) {
                        var_name.push(chars.next().unwrap());
                    } else {
                        while let Some(&nc) = chars.peek() {
                            if nc.is_alphanumeric() || nc == '_' {
                                var_name.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }
                    }
                }
                
                if var_name.is_empty() {
                    result.push('$');
                } else if var_name == "?" || var_name == "LAST" {
                    result.push_str(&self.last_exit_code.to_string());
                } else if var_name == "$" {
                    result.push_str(&std::process::id().to_string());
                } else if var_name == "!" {
                    result.push_str(&jobs.last_job_pid());
                } else if var_name == "#" {
                    result.push_str(&self.script_args.len().to_string());
                } else if var_name == "@" {
                    result.push_str(&self.script_args.join(" "));
                } else if var_name == "0" {
                    result.push_str("ish");
                } else {
                    let mut found = false;
                    let base_idx = *self.function_bases.last().unwrap_or(&0);
                    let mut search_scopes = vec![];
                    for i in (base_idx..self.variables.len()).rev() {
                        search_scopes.push(i);
                    }
                    if base_idx != 0 {
                        search_scopes.push(0); // Also search global scope
                    }
                    
                    for &idx in &search_scopes {
                        let scope = &self.variables[idx];
                        if let Some(val) = scope.get(&var_name) {
                            let mut final_val = val.clone();
                            if chars.peek() == Some(&'[') {
                                chars.next();
                                let mut idx_str = String::new();
                                while let Some(&c) = chars.peek() {
                                    if c == ']' { chars.next(); break; }
                                    idx_str.push(chars.next().unwrap());
                                }
                                if let IshValue::Array(arr) = &final_val {
                                    if let Ok(idx) = idx_str.parse::<usize>() {
                                        if let Some(v) = arr.get(idx) {
                                            final_val = v.clone();
                                        }
                                    }
                                } else if let IshValue::Map(m) = &final_val {
                                    if let Some(v) = m.get(&idx_str) {
                                        final_val = v.clone();
                                    }
                                }
                            }
                            result.push_str(&final_val.to_string());
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        if let Ok(idx) = var_name.parse::<usize>() {
                            if idx > 0 && idx <= self.script_args.len() {
                                result.push_str(&self.script_args[idx - 1]);
                                found = true;
                            } else {
                                // For unbound positional arguments like $1 where args are empty
                                found = true; 
                            }
                        }
                    }
                    if !found {
                        if let Ok(val) = std::env::var(&var_name) {
                            result.push_str(&val);
                            found = true;
                        }
                    }
                    if !found {
                        return Err(IshError::ExecutionError(format!("Variable not found: {}", var_name)));
                    }
                }
            } else {
                result.push(c);
            }
        }
        Ok(result)
    }

    fn resolve_executable(&self, program: &str, args: &[String]) -> (String, Vec<String>) {
        if program.ends_with(".ish") {
            let path = std::path::Path::new(program);
            if path.exists() {
                let mut new_args = vec![program.to_string()];
                new_args.extend_from_slice(args);
                let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("ish"));
                return (exe.to_string_lossy().to_string(), new_args);
            }
        }
        (program.to_string(), args.to_vec())
    }

    fn capture_output(&mut self, node: &AstNode, jobs: &mut JobController) -> Result<String, IshError> {
        let temp_dir = std::env::temp_dir();
        let file_name = format!("ish_cap_{}_{}.tmp", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_path = temp_dir.join(file_name);
        let path_str = temp_path.to_str().unwrap();

        match self.execute_node_with_input(node, "", Some(path_str), jobs) {
            Ok(_) => {
                let out = std::fs::read_to_string(&temp_path).unwrap_or_default();
                let _ = std::fs::remove_file(&temp_path);
                Ok(out.trim().to_string())
            }
            Err(IshError::ExecutionError(e)) if e.starts_with("program not found: ") => {
                let _ = std::fs::remove_file(&temp_path);
                if let AstNodeKind::Command { program, args, .. } = &node.kind {
                    let expected_err = format!("program not found: {}", self.resolve_var(program, jobs)?);
                    if e == expected_err && args.is_empty() {
                        return Ok(self.resolve_var(program, jobs)?);
                    }
                }
                Err(IshError::ExecutionError(e))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&temp_path);
                Err(e)
            }
        }
    }

    pub fn evaluate_node(&mut self, node: &AstNode, jobs: &mut JobController) -> Result<IshValue, IshError> {
        match &node.kind {
            AstNodeKind::StringLiteral(s) => {
                if s.starts_with('$') && !s.contains(" ") && !s.contains("(") && !s.contains("{") {
                    let var_name = &s[1..];
                    let base_idx = *self.function_bases.last().unwrap_or(&0);
                    let mut search_scopes = vec![];
                    for i in (base_idx..self.variables.len()).rev() {
                        search_scopes.push(i);
                    }
                    if base_idx != 0 {
                        search_scopes.push(0);
                    }
                    for i in search_scopes {
                        if let Some(val) = self.variables[i].get(var_name) {
                            return Ok(val.clone());
                        }
                    }
                }
                let resolved = self.resolve_var(s, jobs)?;
                if let Ok(i) = resolved.parse::<i32>() {
                    Ok(IshValue::Int(i))
                } else if let Ok(f) = resolved.parse::<f32>() {
                    Ok(IshValue::Float(f))
                } else if resolved == "true" {
                    Ok(IshValue::Bool(true))
                } else if resolved == "false" {
                    Ok(IshValue::Bool(false))
                } else if resolved == "null" {
                    Ok(IshValue::Null)
                } else {
                    Ok(IshValue::String(resolved))
                }
            }

            AstNodeKind::Condition { left, operator, right } => {
                let left_val = self.evaluate_node(left, jobs)?;
                let right_val = self.evaluate_node(right, jobs)?;
                
                let success = match (left_val, right_val) {
                    (IshValue::Int(l), IshValue::Int(r)) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        ">" => l > r,
                        "<" => l < r,
                        ">=" => l >= r,
                        "<=" => l <= r,
                        _ => false,
                    },
                    (IshValue::Float(l), IshValue::Float(r)) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        ">" => l > r,
                        "<" => l < r,
                        ">=" => l >= r,
                        "<=" => l <= r,
                        _ => false,
                    },
                    (IshValue::Int(l), IshValue::Float(r)) => match operator.as_str() {
                        "==" => (l as f32) == r,
                        "!=" => (l as f32) != r,
                        ">" => (l as f32) > r,
                        "<" => (l as f32) < r,
                        ">=" => (l as f32) >= r,
                        "<=" => (l as f32) <= r,
                        _ => false,
                    },
                    (IshValue::Float(l), IshValue::Int(r)) => match operator.as_str() {
                        "==" => l == (r as f32),
                        "!=" => l != (r as f32),
                        ">" => l > (r as f32),
                        "<" => l < (r as f32),
                        ">=" => l >= (r as f32),
                        "<=" => l <= (r as f32),
                        _ => false,
                    },
                    (l, r) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        _ => false,
                    }
                };
                Ok(IshValue::Bool(success))
            }
            AstNodeKind::Subshell(inner) => {
                let out = self.capture_output(inner, jobs)?;
                if let Ok(i) = out.parse::<i32>() {
                    Ok(IshValue::Int(i))
                } else if let Ok(f) = out.parse::<f32>() {
                    Ok(IshValue::Float(f))
                } else if out == "true" {
                    Ok(IshValue::Bool(true))
                } else if out == "false" {
                    Ok(IshValue::Bool(false))
                } else if out == "null" {
                    Ok(IshValue::Null)
                } else {
                    Ok(IshValue::String(out))
                }
            }
            AstNodeKind::Command { program, args, .. } => {
                if program.starts_with('$') && args.is_empty() {
                    let resolved = self.resolve_var(program, jobs)?;
                    if let Ok(i) = resolved.parse::<i32>() {
                        Ok(IshValue::Int(i))
                    } else if let Ok(f) = resolved.parse::<f32>() {
                        Ok(IshValue::Float(f))
                    } else if resolved == "true" {
                        Ok(IshValue::Bool(true))
                    } else if resolved == "false" {
                        Ok(IshValue::Bool(false))
                    } else if resolved == "null" {
                        Ok(IshValue::Null)
                    } else {
                        Ok(IshValue::String(resolved))
                    }
                } else if args.is_empty() && (program.parse::<f64>().is_ok() || program == "true" || program == "false" || program == "null") {
                    if let Ok(i) = program.parse::<i32>() {
                        Ok(IshValue::Int(i))
                    } else if let Ok(f) = program.parse::<f32>() {
                        Ok(IshValue::Float(f))
                    } else if program == "true" {
                        Ok(IshValue::Bool(true))
                    } else if program == "false" {
                        Ok(IshValue::Bool(false))
                    } else {
                        Ok(IshValue::Null)
                    }
                } else {
                    self.return_value = None;
                    self.execute_node_with_input(node, "", None, jobs)?;
                    if let Some(val) = self.return_value.take() {
                        Ok(val)
                    } else {
                        Ok(IshValue::Null)
                    }
                }
            }
            _ => {
                self.return_value = None;
                self.execute_node_with_input(node, "", None, jobs)?;
                if let Some(val) = self.return_value.take() {
                    Ok(val)
                } else {
                    Ok(IshValue::Null)
                }
            }
        }
    }

    pub fn execute(&mut self, ast: &AstNode, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        // First pass: register all OOP declarations (namespaces, classes, structs)
        self.registry.register_declarations(ast)?;

        // Enforce OOP requirement
        match self.registry.find_entry_point() {
            Ok(program_class_name) => {
                // First, execute all declarations so methods get registered in self.functions
                self.execute_node_with_input(ast, "", None, jobs)?;

                // Now invoke Program::main
                let class_def = self.registry.resolve_class("Program").cloned()
                    .ok_or_else(|| IshError::ExecutionError(
                        "Internal error: Program class disappeared after registration.".to_string()
                    ))?;

                let main_method = class_def.methods.get("main")
                    .ok_or_else(|| IshError::ExecutionError(
                        "Internal error: main method disappeared after registration.".to_string()
                    ))?;

                self.variables.push(HashMap::new());
                self.function_bases.push(self.variables.len() - 1);
                let prev_class = self.current_class.clone();
                self.current_class = Some(program_class_name);

                let mut success = true;
                for stmt in &main_method.body {
                    success = self.execute_node_with_input(stmt, "", None, jobs)?;
                    if self.returning { break; }
                }

                // Retrieve return value (exit code)
                let ret_val = self.return_value.take();
                self.returning = false;
                self.function_bases.pop();
                self.variables.pop();
                self.current_class = prev_class;

                if let Some(IshValue::Int(code)) = ret_val {
                    self.last_exit_code = code;
                }

                Ok(success)
            }
            Err(_) => {
                Err(IshError::ParseError(
                    "Script must define 'public static class Program' with a 'public static func main()' method.".to_string()
                ))
            }
        }
    }

    fn execute_node_with_input(&mut self, node: &AstNode, input: &str, out_file: Option<&str>, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        match &node.kind {
            AstNodeKind::Subshell(inner) => {
                let val = self.evaluate_node(inner, jobs)?;
                self.return_value = Some(val);
                Ok(true)
            }
            AstNodeKind::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } => {
                let resolved_program = self.resolve_var(program, jobs)?;
                let mut resolved_args = Vec::new();
                for arg in args {
                                resolved_args.push(self.resolve_var(arg, jobs)?);
                }

                if let Some(AstNode { kind: AstNodeKind::Function { params, body, .. }, .. }) = self.functions.get(&resolved_program).cloned() {
                    self.variables.push(HashMap::new());
                    self.function_bases.push(self.variables.len() - 1);
                    
                    if let Some(scope) = self.variables.last_mut() {
                        for (i, param) in params.iter().enumerate() {
                            if i < resolved_args.len() {
                                scope.insert(param.clone(), IshValue::String(resolved_args[i].clone()));
                            }
                        }
                        for (i, arg) in resolved_args.iter().enumerate() {
                            scope.insert((i + 1).to_string(), IshValue::String(arg.clone()));
                        }
                    }

                    let mut success = true;
                    for stmt in body.iter() {
                        if self.returning { break; }
                        success = self.execute_node_with_input(stmt, input, out_file, jobs)?;
                    }

                    self.returning = false;
                    self.function_bases.pop();
                    self.variables.pop();
                    self.last_exit_code = if success { 0 } else { 1 };
                    return Ok(success);
                }

                let (final_program, final_args) = self.resolve_executable(&resolved_program, &resolved_args);

                for provider in &self.stdlib_providers {
                    if provider.handles_command(&final_program) {
                        match provider.execute(&final_program, &final_args) {
                            Ok(output) => {
                                self.last_exit_code = 0;
                                let out_str = if output.ends_with('\n') { output.clone() } else { format!("{}\n", output) };
                                
                                if let Some(path) = out_file {
                                    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                    let _ = file.write_all(output.as_bytes());
                                } else if let Some(path) = redirect_to {
                                    if path != "DevNull" {
                                        let mut file = File::create(path)?;
                                        let _ = file.write_all(output.as_bytes());
                                    }
                                } else if let Some(path) = append_to {
                                    if path != "DevNull" {
                                        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                        let _ = file.write_all(output.as_bytes());
                                    }
                                } else {
                                     if !out_str.trim().is_empty() {
                                        print!("{}", out_str);
                                        let _ = std::io::stdout().flush();
                                    }
                                }
                                return Ok(true);
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                }

                let mut internal_result = match final_program.as_str() {
                    "jobs" => Ok(Some(crate::core::ast::IshValue::String(jobs.list_jobs()))),
                    "fg" => {
                        if final_args.is_empty() {
                            Err("fg error: expected job id".to_string())
                        } else if let Ok(id) = final_args[0].parse::<u32>() {
                            match jobs.wait_job(id) {
                                Ok(msg) => Ok(Some(crate::core::ast::IshValue::String(msg + "\n"))),
                                Err(e) => Err(e),
                            }
                        } else {
                            Err("fg error: invalid job id".to_string())
                        }
                    }
                    "kill" => {
                        if final_args.is_empty() {
                            Err("kill error: expected job id".to_string())
                        } else if let Ok(id) = final_args[0].parse::<u32>() {
                            match jobs.kill_job(id) {
                                Ok(msg) => Ok(Some(crate::core::ast::IshValue::String(msg + "\n"))),
                                Err(e) => Err(e),
                            }
                        } else {
                            Err("kill error: invalid job id".to_string())
                        }
                    }
                    _ => crate::core::utils::execute_internal(&final_program, &final_args)
                };

                if let Ok(None) = internal_result {
                    if let Ok(Some(translated)) = crate::core::os_interceptor::translate_and_execute(&final_program, &final_args) {
                        internal_result = Ok(Some(crate::core::ast::IshValue::String(translated)));
                    }
                }

                match internal_result {
                    Ok(Some(output_val)) => {
                        self.last_exit_code = 0;
                        if let Some(path) = out_file {
                            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            let _ = file.write_all(output_val.to_string().as_bytes());
                        } else if let Some(path) = redirect_to {
                            if path != "DevNull" {
                                let mut file = File::create(path)?;
                                let _ = file.write_all(output_val.to_string().as_bytes());
                            }
                        } else if let Some(path) = append_to {
                            if path != "DevNull" {
                                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                let _ = file.write_all(output_val.to_string().as_bytes());
                            }
                        } else {
                            let output = output_val.to_string();
                            if !output.trim().is_empty() {
                                if let crate::core::ast::IshValue::Array(_) | crate::core::ast::IshValue::Map(_) = output_val {
                                    // Structured output will be printed with its custom display
                                    print!("{}", output);
                                } else {
                                    let out_str = if output.ends_with('\n') { output.clone() } else { format!("{}\n", output) };
                                    print!("{}", out_str);
                                }
                                let _ = std::io::stdout().flush();
                            }
                        }
                        return Ok(true);
                    }
                    Err(e) => {
                        self.last_exit_code = 1;
                        if *merge_err {
                            if let Some(path) = out_file {
                                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                let _ = file.write_all(e.as_bytes());
                            } else if let Some(path) = redirect_to {
                                if path != "DevNull" {
                                    let mut file = File::create(path)?;
                                    let _ = file.write_all(e.as_bytes());
                                }
                            } else {
                                eprintln!("{}", e);
                            }
                        } else {
                            eprintln!("{}", e);
                        }
                        return Ok(false);
                    }
                    Ok(None) => {}
                }

                let mut cmd = Command::new(&final_program);
                cmd.args(&final_args);

                if let Some(path) = out_file {
                    let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                    cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                    if *merge_err { cmd.stderr(Stdio::from(file)); }
                } else if let Some(path) = redirect_to {
                    if path == "DevNull" {
                        cmd.stdout(Stdio::null());
                        if *merge_err { cmd.stderr(Stdio::null()); }
                    } else {
                        let file = File::create(path)?;
                        cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                        if *merge_err { cmd.stderr(Stdio::from(file)); }
                    }
                } else if let Some(path) = append_to {
                    if path == "DevNull" {
                        cmd.stdout(Stdio::null());
                        if *merge_err { cmd.stderr(Stdio::null()); }
                    } else {
                        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                        cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                        if *merge_err { cmd.stderr(Stdio::from(file)); }
                    }
                } else {
                    cmd.stdout(Stdio::inherit());
                }
                
                if !*merge_err || (redirect_to.is_none() && append_to.is_none()) {
                    cmd.stderr(Stdio::inherit());
                }

                if let Some(path) = redirect_from {
                    if path == "DevNull" {
                        cmd.stdin(Stdio::null());
                    } else {
                        let file = File::open(path)?;
                        cmd.stdin(Stdio::from(file));
                    }
                } else if read_doc.is_some() {
                    cmd.stdin(Stdio::piped());
                } else if !input.is_empty() {
                    cmd.stdin(Stdio::piped());
                } else {
                    cmd.stdin(Stdio::inherit());
                }

                let mut child = cmd.spawn().map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        IshError::ExecutionError(format!("program not found: {}", final_program))
                    } else {
                        IshError::ExecutionError(e.to_string())
                    }
                })?;

                if let Some(doc) = read_doc {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(doc.as_bytes());
                    }
                } else if !input.is_empty() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(input.as_bytes());
                    }
                }

                let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });

                Ok(status.success())
            }
            AstNodeKind::Sequential(left, right) => {
                let _s1 = self.execute_node_with_input(left, input, out_file, jobs)?;
                let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                Ok(s2)
            }
            AstNodeKind::AndThen(left, right) => {
                let success = self.execute_node_with_input(left, input, out_file, jobs)?;
                if success {
                    let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                    Ok(s2)
                } else {
                    Ok(false)
                }
            }
            AstNodeKind::OrElse(left, right) => {
                let success = self.execute_node_with_input(left, input, out_file, jobs)?;
                if !success {
                    let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                    Ok(s2)
                } else {
                    Ok(true)
                }
            }
            AstNodeKind::Pipeline(nodes) => {
                if nodes.is_empty() {
                    return Ok(true);
                }

                let mut previous_stdout: Option<std::process::ChildStdout> = None;
                let mut internal_output: Option<crate::core::ast::IshValue> = None;
                let mut children = Vec::new();
                let mut last_cmd_success = true;

                for (i, node) in nodes.iter().enumerate() {
                    if let AstNodeKind::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } = &node.kind {
                        let resolved_program = self.resolve_var(program, jobs)?;
                        let mut resolved_args = Vec::new();
                        for arg in args {
                            resolved_args.push(self.resolve_var(arg, jobs)?);
                        }

                        if let Some(AstNode { kind: AstNodeKind::Function { params, body, .. }, .. }) = self.functions.get(&resolved_program).cloned() {
                            self.variables.push(HashMap::new());
                            self.function_bases.push(self.variables.len() - 1);
                            if let Some(scope) = self.variables.last_mut() {
                                for (i, param) in params.iter().enumerate() {
                                    if i < resolved_args.len() {
                                        scope.insert(param.clone(), IshValue::String(resolved_args[i].clone()));
                                    }
                                }
                                for (i, arg) in resolved_args.iter().enumerate() {
                                    scope.insert((i + 1).to_string(), IshValue::String(arg.clone()));
                                }
                            }
                            let mut success = true;
                            for stmt in body.iter() {
                                if self.returning { break; }
                                // FIXME: We should handle piped I/O for function pipelines, but for now just basic out_file works
                                success = self.execute_node_with_input(stmt, input, out_file, jobs)?;
                            }
                            self.returning = false;
                            self.function_bases.pop();
                            self.variables.pop();
                            self.last_exit_code = if success { 0 } else { 1 };
                            last_cmd_success = success;
                            continue;
                        }

                        let (final_program, final_args) = self.resolve_executable(&resolved_program, &resolved_args);

                        let mut stdlib_handled = false;
                        for provider in &self.stdlib_providers {
                            if provider.handles_command(&final_program) {
                                stdlib_handled = true;
                                match provider.execute(&final_program, &final_args) {
                                    Ok(output) => {
                                        self.last_exit_code = 0;
                                        last_cmd_success = true;
                                        if i < nodes.len() - 1 {
                                            // Handle Stdlib provider output strings by parsing them speculatively
                                            let parsed = match serde_json::from_str::<serde_json::Value>(&output) {
                                                Ok(v) => crate::core::ast::IshValue::from(v),
                                                Err(_) => crate::core::ast::IshValue::String(output.clone()),
                                            };
                                            internal_output = Some(parsed);
                                            let _ = previous_stdout.take();
                                        } else {
                                            if let Some(path) = out_file {
                                                let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                                let _ = file.write_all(output.as_bytes());
                                            } else if let Some(path) = redirect_to {
                                                if path != "DevNull" {
                                                    let mut file = File::create(path)?;
                                                    let _ = file.write_all(output.as_bytes());
                                                }
                                            } else {
                                                let out_str = if output.ends_with('\n') { output.clone() } else { format!("{}\n", output) };
                                                if !out_str.trim().is_empty() {
                                                    print!("{}", out_str);
                                                    let _ = std::io::stdout().flush();
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        return Err(e);
                                    }
                                }
                                break;
                            }
                        }
                        if stdlib_handled {
                            continue;
                        }

                        match crate::core::utils::execute_internal(&final_program, &final_args) {
                            Ok(Some(output_val)) => {
                                self.last_exit_code = 0;
                                last_cmd_success = true;
                                if i < nodes.len() - 1 {
                                    internal_output = Some(output_val);
                                    let _ = previous_stdout.take();
                                } else {
                                    let output_str = output_val.to_string();
                                    if let Some(path) = out_file {
                                        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                        let _ = file.write_all(output_str.as_bytes());
                                    } else if let Some(path) = redirect_to {
                                        if path != "DevNull" {
                                            let mut file = File::create(path)?;
                                            let _ = file.write_all(output_str.as_bytes());
                                        }
                                    } else {
                                        print!("{}", output_str);
                                        let _ = std::io::stdout().flush();
                                    }
                                }
                                continue;
                            }
                            Err(e) => {
                                self.last_exit_code = 1;
                                last_cmd_success = false;
                                let err_msg = if *merge_err {
                                    serde_json::to_string(&serde_json::json!({
                                        "error": e,
                                        "status": 1,
                                    })).unwrap_or(e)
                                } else {
                                    e
                                };
                                eprintln!("{}", err_msg);
                                break;
                            }
                            Ok(None) => {}
                        }

                        let mut cmd = Command::new(&final_program);
                        cmd.args(&final_args);

                        // stdin precedence: explicit redirect > read_doc > pipe from previous > input string
                        if let Some(path) = redirect_from {
                            let _ = previous_stdout.take(); // Discard piped input
                            if path == "DevNull" {
                                cmd.stdin(Stdio::null());
                            } else {
                                let file = File::open(path)?;
                                cmd.stdin(Stdio::from(file));
                            }
                        } else if read_doc.is_some() {
                            let _ = previous_stdout.take(); // Discard piped input
                            cmd.stdin(Stdio::piped());
                        } else if let Some(out_val) = internal_output.take() {
                            cmd.stdin(Stdio::piped());
                            internal_output = Some(out_val); // put it back to write later
                        } else if let Some(stdout) = previous_stdout.take() {
                            cmd.stdin(Stdio::from(stdout));
                        } else if i == 0 && !input.is_empty() {
                            cmd.stdin(Stdio::piped());
                        } else {
                            cmd.stdin(Stdio::inherit());
                        }

                        // stdout precedence: explicit out_file > explicit redirect > append > pipe to next > inherit
                        let mut piped_stdout = false;
                        if let Some(path) = out_file.filter(|_| i == nodes.len() - 1) {
                            let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                            if *merge_err { cmd.stderr(Stdio::from(file)); }
                        } else if let Some(path) = redirect_to {
                            if path == "DevNull" {
                                cmd.stdout(Stdio::null());
                                if *merge_err { cmd.stderr(Stdio::null()); }
                            } else {
                                let file = File::create(path)?;
                                cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                                if *merge_err { cmd.stderr(Stdio::from(file)); }
                            }
                        } else if let Some(path) = append_to {
                            if path == "DevNull" {
                                cmd.stdout(Stdio::null());
                                if *merge_err { cmd.stderr(Stdio::null()); }
                            } else {
                                let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                                if *merge_err { cmd.stderr(Stdio::from(file)); }
                            }
                        } else if i < nodes.len() - 1 {
                            cmd.stdout(Stdio::piped());
                            piped_stdout = true;
                        } else {
                            cmd.stdout(Stdio::inherit());
                        }

                        if !*merge_err || piped_stdout || (redirect_to.is_none() && append_to.is_none()) {
                            cmd.stderr(Stdio::inherit());
                        }

                        let mut child = cmd.spawn().map_err(|e| {
                            if e.kind() == std::io::ErrorKind::NotFound {
                                IshError::ExecutionError(format!("program not found: {}", final_program))
                            } else {
                                IshError::ExecutionError(e.to_string())
                            }
                        })?;

                        if let Some(doc) = read_doc {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(doc.as_bytes());
                            }
                        } else if let Some(out_val) = internal_output.take() {
                            if let Some(mut stdin) = child.stdin.take() {
                                let mut output_str = out_val.to_string();
                                if let crate::core::ast::IshValue::Map(_) | crate::core::ast::IshValue::Array(_) = &out_val {
                                    // if it's a native structure, serialize to json string
                                    output_str = serde_json::to_string(&Into::<serde_json::Value>::into(out_val)).unwrap_or(output_str);
                                }
                                let _ = stdin.write_all(output_str.as_bytes());
                            }
                        } else if i == 0 && !input.is_empty() && previous_stdout.is_none() {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(input.as_bytes());
                            }
                        }

                        if piped_stdout {
                            previous_stdout = child.stdout.take();
                        }

                        children.push(child);
                    } else {
                        let success = self.execute_node_with_input(node, "", out_file, jobs)?;
                        last_cmd_success = success;
                        let _ = previous_stdout.take();
                    }
                }

                for mut child in children {
                    let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                    self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });
                    last_cmd_success = status.success();
                }

                return Ok(last_cmd_success);
            }
            AstNodeKind::Background(inner) => {
                if let AstNodeKind::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } = &inner.kind {
                    let resolved_program = self.resolve_var(program, jobs)?;
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_var(arg, jobs)?);
                    }
                    if self.functions.contains_key(&resolved_program) {
                        return Err(IshError::ExecutionError("Cannot run custom functions in background via AstNodeKind::Background currently".into()));
                    }

                    let (final_program, final_args) = self.resolve_executable(&resolved_program, &resolved_args);
                    let mut cmd = Command::new(&final_program);
                    cmd.args(&final_args);

                    if let Some(path) = out_file {
                        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                        cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                        if *merge_err { cmd.stderr(Stdio::from(file)); }
                    } else if let Some(path) = redirect_to {
                        if path == "DevNull" {
                            cmd.stdout(Stdio::null());
                            if *merge_err { cmd.stderr(Stdio::null()); }
                        } else {
                            let file = File::create(path)?;
                            cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                            if *merge_err { cmd.stderr(Stdio::from(file)); }
                        }
                    } else if let Some(path) = append_to {
                        if path == "DevNull" {
                            cmd.stdout(Stdio::null());
                            if *merge_err { cmd.stderr(Stdio::null()); }
                        } else {
                            let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            cmd.stdout(Stdio::from(file.try_clone().unwrap()));
                            if *merge_err { cmd.stderr(Stdio::from(file)); }
                        }
                    } else {
                        cmd.stdout(Stdio::inherit());
                    }
                    
                    if !*merge_err || (redirect_to.is_none() && append_to.is_none()) {
                        cmd.stderr(Stdio::inherit());
                    }

                    if let Some(path) = redirect_from {
                        if path == "DevNull" {
                            cmd.stdin(Stdio::null());
                        } else {
                            let file = File::open(path)?;
                            cmd.stdin(Stdio::from(file));
                        }
                    } else if read_doc.is_some() {
                        cmd.stdin(Stdio::piped());
                    } else {
                        cmd.stdin(Stdio::null());
                    }

                    match cmd.spawn() {
                        Ok(mut child) => {
                            if let Some(doc) = read_doc {
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(doc.as_bytes());
                                }
                            }
                            let job_id = jobs.add_job(child);
                            println!("[Job started in background] ID: {}", job_id);
                            Ok(true)
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::NotFound {
                                Err(IshError::ExecutionError(format!("program not found: {}", final_program)))
                            } else {
                                Err(IshError::ExecutionError(e.to_string()))
                            }
                        }
                    }
                } else {
                    Err(IshError::ExecutionError("Only simple commands can run in background currently".into()))
                }
            }
            AstNodeKind::Condition { left, operator, right } => {
                let left_val = self.evaluate_node(left, jobs)?;
                let right_val = self.evaluate_node(right, jobs)?;
                
                let success = match (left_val, right_val) {
                    (IshValue::Int(l), IshValue::Int(r)) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        ">" => l > r,
                        "<" => l < r,
                        ">=" => l >= r,
                        "<=" => l <= r,
                        _ => false,
                    },
                    (IshValue::Float(l), IshValue::Float(r)) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        ">" => l > r,
                        "<" => l < r,
                        ">=" => l >= r,
                        "<=" => l <= r,
                        _ => false,
                    },
                    (IshValue::Int(l), IshValue::Float(r)) => match operator.as_str() {
                        "==" => (l as f32) == r,
                        "!=" => (l as f32) != r,
                        ">" => (l as f32) > r,
                        "<" => (l as f32) < r,
                        ">=" => (l as f32) >= r,
                        "<=" => (l as f32) <= r,
                        _ => false,
                    },
                    (IshValue::Float(l), IshValue::Int(r)) => match operator.as_str() {
                        "==" => l == (r as f32),
                        "!=" => l != (r as f32),
                        ">" => l > (r as f32),
                        "<" => l < (r as f32),
                        ">=" => l >= (r as f32),
                        "<=" => l <= (r as f32),
                        _ => false,
                    },
                    (l, r) => match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        _ => false,
                    }
                };
                
                self.last_exit_code = if success { 0 } else { 1 };
                Ok(success)
            }

            AstNodeKind::Assignment { variable, value, is_declaration } => {
                let val = match &value.kind {
                    AstNodeKind::Array(items) => {
                        let mut arr = Vec::new();
                        for item in items {
                            arr.push(self.evaluate_node(item, jobs)?);
                        }
                        IshValue::Array(arr)
                    }
                    AstNodeKind::Map(items) => {
                        let mut m = HashMap::new();
                        for (k, v) in items {
                            m.insert(k.clone(), self.evaluate_node(v, jobs)?);
                        }
                        IshValue::Map(m)
                    }
                    _ => self.evaluate_node(value, jobs)?,
                };
                
                if *is_declaration {
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), val);
                    }
                } else {
                    let mut found = false;
                    for scope in self.variables.iter_mut().rev() {
                        if scope.contains_key(variable) {
                            scope.insert(variable.clone(), val.clone());
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return Err(IshError::ExecutionError(format!("Variable '{}' is not declared. Use 'declare {} = ...' to declare it.", variable, variable)));
                    }
                }
                Ok(true)
            }
            AstNodeKind::If { condition, body, else_body } => {
                let success = match self.evaluate_node(condition, jobs)? {
                    IshValue::Bool(b) => b,
                    IshValue::Int(i) => i != 0,
                    IshValue::Float(f) => f != 0.0,
                    IshValue::String(s) => !s.is_empty() && s != "false" && s != "null",
                    IshValue::Null => false,
                    _ => true,
                };
                let mut block_success = true;
                if success {
                    self.variables.push(HashMap::new());
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    self.variables.pop();
                } else if let Some(else_stmts) = else_body {
                    self.variables.push(HashMap::new());
                    for stmt in else_stmts {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    self.variables.pop();
                }
                Ok(block_success)
            }
            AstNodeKind::For { variable, iterable, body } => {
                let iterable_val = self.evaluate_node(iterable, jobs)?;
                let items: Vec<String> = match iterable_val {
                    IshValue::Array(arr) => arr.iter().map(|v| v.to_string()).collect(),
                    IshValue::String(s) => s.split_whitespace().map(|s| s.to_string()).collect(),
                    _ => vec![iterable_val.to_string()],
                };
                let mut block_success = true;
                for item in items {
                    if self.returning || self.breaking { break; }
                    self.variables.push(HashMap::new());
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), IshValue::String(item));
                    }
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    self.variables.pop();
                    if self.continuing {
                        self.continuing = false;
                    }
                    if self.breaking || self.returning { break; }
                }
                if self.breaking { self.breaking = false; }
                Ok(block_success)
            }
            AstNodeKind::While { condition, body } => {
                let mut block_success = true;
                while match self.evaluate_node(condition, jobs)? {
                    IshValue::Bool(b) => b,
                    IshValue::Int(i) => i != 0,
                    IshValue::Float(f) => f != 0.0,
                    IshValue::String(s) => !s.is_empty() && s != "false" && s != "null",
                    IshValue::Null => false,
                    _ => true,
                } {
                    if self.returning || self.breaking { break; }
                    self.variables.push(HashMap::new());
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    self.variables.pop();
                    if self.continuing {
                        self.continuing = false;
                    }
                    if self.breaking || self.returning { break; }
                }
                if self.breaking { self.breaking = false; }
                Ok(block_success)
            }
            AstNodeKind::TryCatch { try_body, error_var, catch_body } => {
                let mut success = true;
                self.variables.push(HashMap::new());
                for stmt in try_body {
                    match self.execute_node_with_input(stmt, "", out_file, jobs) {
                        Ok(s) => {
                            if !s {
                                success = false;
                                break;
                            }
                        }
                        Err(e) => {
                            self.variables.pop(); // pop try block scope
                            
                            self.variables.push(HashMap::new()); // push catch block scope
                            if let Some(scope) = self.variables.last_mut() {
                                scope.insert(error_var.clone(), IshValue::String(e.to_string()));
                            }
                            let mut catch_success = true;
                            for c_stmt in catch_body {
                                match self.execute_node_with_input(c_stmt, "", out_file, jobs) {
                                    Ok(s) => if !s { catch_success = false; },
                                    Err(_) => { catch_success = false; break; },
                                }
                            }
                            self.variables.pop(); // pop catch block scope
                            self.last_exit_code = if catch_success { 0 } else { 1 };
                            return Ok(catch_success);
                        }
                    }
                    if self.returning || self.breaking || self.continuing { break; }
                }
                self.variables.pop();
                self.last_exit_code = if success { 0 } else { 1 };
                Ok(success)
            }
            AstNodeKind::Break => {
                self.breaking = true;
                Ok(true)
            }
            AstNodeKind::Continue => {
                self.continuing = true;
                Ok(true)
            }
            AstNodeKind::Function { name, .. } => {
                self.functions.insert(name.clone(), node.clone());
                Ok(true)
            }
            AstNodeKind::Array(_) | AstNodeKind::Map(_) => {
                Ok(true)
            }
            AstNodeKind::StringLiteral(s) => {
                let resolved = self.resolve_var(s, jobs)?;
                if let Some(path) = out_file {
                    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                    let _ = file.write_all(resolved.as_bytes());
                } else if !resolved.is_empty() {
                    print!("{}", resolved);
                    let _ = std::io::stdout().flush();
                }
                Ok(true)
            }
            AstNodeKind::Return(inner) => {
                let val = self.evaluate_node(inner, jobs)?;
                self.return_value = Some(val);
                self.returning = true;
                Ok(true)
            }
            AstNodeKind::Parallel(left, right) => {
                let mut exec_left = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    function_bases: vec![0],
                    return_value: None,
                    functions: self.functions.clone(),
                    returning: false,
                    breaking: false,
                    continuing: false,
                    stdlib_providers: vec![Box::new(crate::core::stdlib::IshStr)],
                    registry: self.registry.clone(),
                    current_class: self.current_class.clone(),
                };
                let mut exec_right = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    function_bases: vec![0],
                    return_value: None,
                    functions: self.functions.clone(),
                    returning: false,
                    breaking: false,
                    continuing: false,
                    stdlib_providers: vec![Box::new(crate::core::stdlib::IshStr)],
                    registry: self.registry.clone(),
                    current_class: self.current_class.clone(),
                };
                let left_node = left.clone();
                let right_node = right.clone();
                let input_str = input.to_string();
                let out_str = out_file.map(|s| s.to_string());
                
                let handle = std::thread::spawn(move || {
                    let mut jobs = JobController::new();
                    exec_left.execute_node_with_input(&left_node, &input_str, out_str.as_deref(), &mut jobs)
                });
                
                let mut right_jobs = JobController::new();
                let s2 = exec_right.execute_node_with_input(&right_node, "", out_file, &mut right_jobs)?;
                
                let s1 = handle.join().unwrap_or(Ok(false))?;
                
                Ok(s1 && s2)
            }

            // ---- OOP: Namespace, Class, Struct Declarations ----
            // These are handled during first-pass registration; at execution time
            // we simply recurse into the body so inner functions/statements register normally.
            AstNodeKind::NamespaceDecl { body, .. } => {
                for stmt in body {
                    self.execute_node_with_input(stmt, input, out_file, jobs)?;
                    if self.returning || self.breaking || self.continuing { break; }
                }
                Ok(true)
            }
            AstNodeKind::ClassDecl { name, methods, .. } => {
                // Register methods as qualified functions (ClassName::method_name)
                for method_node in methods {
                    if let AstNodeKind::Function { name: mname, .. } = &method_node.kind {
                        let qualified = format!("{}::{}", name, mname);
                        self.functions.insert(qualified, method_node.clone());
                    }
                }
                Ok(true)
            }
            AstNodeKind::StructDecl { .. } => {
                // Struct definitions are fully handled by the registry first-pass
                Ok(true)
            }

            // ---- OOP: Object Instantiation ----
            AstNodeKind::ObjectInstantiation { class_name, args } => {
                let class_def = self.registry.resolve_class(class_name).cloned()
                    .ok_or_else(|| IshError::ExecutionError(format!(
                        "Class '{}' is not defined.", class_name
                    )))?;

                if class_def.is_static {
                    return Err(IshError::ExecutionError(format!(
                        "Cannot instantiate static class '{}'.", class_name
                    )));
                }

                // Check access
                Registry::check_access(&class_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;

                // Build default properties from field definitions
                let mut properties = HashMap::new();
                for (fname, fdef) in &class_def.fields {
                    let val = if let Some(default_node) = &fdef.default_value {
                        self.evaluate_node(default_node, jobs)?
                    } else {
                        IshValue::Null
                    };
                    properties.insert(fname.clone(), val);
                }

                // If there's a constructor, call it with the provided args
                let mut eval_args = Vec::new();
                for arg_node in args {
                    eval_args.push(self.evaluate_node(arg_node, jobs)?);
                }

                if let Some(constructor) = class_def.methods.get(class_name) {
                    // Push a scope for constructor execution
                    self.variables.push(HashMap::new());
                    self.function_bases.push(self.variables.len() - 1);
                    if let Some(scope) = self.variables.last_mut() {
                        // Bind `this` as properties map
                        scope.insert("this".to_string(), IshValue::Object {
                            class_name: class_name.clone(),
                            properties: properties.clone(),
                        });
                        for (i, param) in constructor.params.iter().enumerate() {
                            if i < eval_args.len() {
                                scope.insert(param.clone(), eval_args[i].clone());
                            }
                        }
                    }
                    let prev_class = self.current_class.clone();
                    self.current_class = Some(class_def.qualified_name.clone());
                    for stmt in &constructor.body {
                        self.execute_node_with_input(stmt, "", None, jobs)?;
                        if self.returning { break; }
                    }
                    // Extract potentially-modified `this`
                    if let Some(scope) = self.variables.last() {
                        if let Some(IshValue::Object { properties: p, .. }) = scope.get("this") {
                            properties = p.clone();
                        }
                    }
                    self.returning = false;
                    self.function_bases.pop();
                    self.variables.pop();
                    self.current_class = prev_class;
                }

                self.return_value = Some(IshValue::Object {
                    class_name: class_name.clone(),
                    properties,
                });
                Ok(true)
            }

            // ---- OOP: Method Call ----
            AstNodeKind::MethodCall { object, method_name, args } => {
                let obj_val = self.evaluate_node(object, jobs)?;

                match obj_val {
                    IshValue::Object { ref class_name, .. } => {
                        let class_def = self.registry.resolve_class(class_name).cloned()
                            .ok_or_else(|| IshError::ExecutionError(format!(
                                "Class '{}' is not defined (method call on object).", class_name
                            )))?;

                        let method = class_def.methods.get(method_name.as_str())
                            .ok_or_else(|| IshError::ExecutionError(format!(
                                "Method '{}' not found on class '{}'.", method_name, class_name
                            )))?;

                        // Check access specifier
                        Registry::check_access(&method.access, self.current_class.as_deref(), &class_def.qualified_name)?;

                        // Evaluate arguments
                        let mut eval_args = Vec::new();
                        for arg_node in args {
                            eval_args.push(self.evaluate_node(arg_node, jobs)?);
                        }

                        // Push method scope
                        self.variables.push(HashMap::new());
                        self.function_bases.push(self.variables.len() - 1);
                        if let Some(scope) = self.variables.last_mut() {
                            scope.insert("this".to_string(), obj_val.clone());
                            for (i, param) in method.params.iter().enumerate() {
                                if i < eval_args.len() {
                                    scope.insert(param.clone(), eval_args[i].clone());
                                }
                            }
                        }

                        let prev_class = self.current_class.clone();
                        self.current_class = Some(class_def.qualified_name.clone());

                        let mut success = true;
                        for stmt in &method.body {
                            success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                            if self.returning { break; }
                        }

                        let ret_val = self.return_value.take();
                        self.returning = false;
                        self.function_bases.pop();
                        self.variables.pop();
                        self.current_class = prev_class;

                        if let Some(val) = ret_val {
                            self.return_value = Some(val);
                        }

                        self.last_exit_code = if success { 0 } else { 1 };
                        Ok(success)
                    }
                    _ => {
                        // For non-object values, try calling as a qualified static method (e.g. ClassName::method)
                        Err(IshError::ExecutionError(format!(
                            "Cannot call method '{}' on non-object value '{}'.", method_name, obj_val.to_string()
                        )))
                    }
                }
            }

            // ---- OOP: Property Access ----
            AstNodeKind::PropertyAccess { object, property_name } => {
                let obj_val = self.evaluate_node(object, jobs)?;

                match &obj_val {
                    IshValue::Object { class_name, properties } => {
                        // Check field access
                        if let Some(class_def) = self.registry.resolve_class(class_name) {
                            if let Some(field_def) = class_def.fields.get(property_name.as_str()) {
                                Registry::check_access(&field_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;
                            }
                        }

                        let val = properties.get(property_name.as_str())
                            .cloned()
                            .unwrap_or(IshValue::Null);

                        let output = val.to_string();
                        if let Some(path) = out_file {
                            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            let _ = file.write_all(output.as_bytes());
                        } else if !output.is_empty() {
                            print!("{}", output);
                            let _ = std::io::stdout().flush();
                        }
                        self.return_value = Some(val);
                        Ok(true)
                    }
                    IshValue::Map(m) => {
                        let val = m.get(property_name.as_str())
                            .cloned()
                            .unwrap_or(IshValue::Null);
                        let output = val.to_string();
                        if let Some(path) = out_file {
                            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            let _ = file.write_all(output.as_bytes());
                        } else if !output.is_empty() {
                            print!("{}", output);
                            let _ = std::io::stdout().flush();
                        }
                        self.return_value = Some(val);
                        Ok(true)
                    }
                    _ => Err(IshError::ExecutionError(format!(
                        "Cannot access property '{}' on non-object value '{}'.", property_name, obj_val.to_string()
                    ))),
                }
            }
        }
    }
}

