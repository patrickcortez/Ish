use crate::core::ast::{AstNode, AstNodeKind, IshValue};
use crate::error::IshError;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;
use crate::managers::job_controller::JobController;

pub struct Executor {
    pub script_args: Vec<String>,
    pub last_exit_code: i32,
    pub variables: Vec<HashMap<String, IshValue>>,
    pub functions: HashMap<String, AstNode>,
    pub returning: bool,
    pub breaking: bool,
    pub continuing: bool,
}

impl Executor {
    pub fn new(script_args: Vec<String>) -> Self {
        Self { 
            script_args, 
            last_exit_code: 0,
            variables: vec![HashMap::new()],
            functions: HashMap::new(),
            returning: false,
            breaking: false,
            continuing: false,
        }
    }

    fn resolve_var(&self, s: &str) -> String {
        let mut result = String::new();
        let mut chars = s.chars().peekable();
        
        while let Some(c) = chars.next() {
            if c == '$' {
                let mut var_name = String::new();
                while let Some(&nc) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        var_name.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                
                if var_name == "LAST" {
                    result.push_str(&self.last_exit_code.to_string());
                } else if var_name.is_empty() {
                    result.push('$');
                } else {
                    let mut found = false;
                    for scope in self.variables.iter().rev() {
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
                            }
                        }
                    }
                    if !found {
                        if let Ok(val) = std::env::var(&var_name) {
                            result.push_str(&val);
                        }
                    }
                }
            } else {
                result.push(c);
            }
        }
        result
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
                    let expected_err = format!("program not found: {}", self.resolve_var(program));
                    if e == expected_err && args.is_empty() {
                        return Ok(self.resolve_var(program));
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

    pub fn execute(&mut self, ast: &AstNode, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        self.execute_node_with_input(ast, "", None, jobs)
    }

    fn execute_node_with_input(&mut self, node: &AstNode, input: &str, out_file: Option<&str>, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        match &node.kind {
            AstNodeKind::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } => {
                let resolved_program = self.resolve_var(program);
                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(self.resolve_var(arg));
                }

                if let Some(AstNode { kind: AstNodeKind::Function { params, body, .. }, .. }) = self.functions.get(&resolved_program).cloned() {
                    self.variables.push(HashMap::new());
                    
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
                    self.variables.pop();
                    self.last_exit_code = if success { 0 } else { 1 };
                    return Ok(success);
                }

                let (final_program, final_args) = self.resolve_executable(&resolved_program, &resolved_args);

                let internal_result = match final_program.as_str() {
                    "jobs" => Ok(Some(jobs.list_jobs())),
                    "fg" => {
                        if final_args.is_empty() {
                            Err("fg error: expected job id".to_string())
                        } else if let Ok(id) = final_args[0].parse::<u32>() {
                            match jobs.wait_job(id) {
                                Ok(msg) => Ok(Some(msg + "\n")),
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
                                Ok(msg) => Ok(Some(msg + "\n")),
                                Err(e) => Err(e),
                            }
                        } else {
                            Err("kill error: invalid job id".to_string())
                        }
                    }
                    _ => crate::core::utils::execute_internal(&final_program, &final_args)
                };

                match internal_result {
                    Ok(Some(output)) => {
                        self.last_exit_code = 0;
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
                            print!("{}", output);
                            let _ = std::io::stdout().flush();
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
                let mut internal_output: Option<String> = None;
                let mut children = Vec::new();
                let mut last_cmd_success = true;

                for (i, node) in nodes.iter().enumerate() {
                    if let AstNodeKind::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } = &node.kind {
                        let resolved_program = self.resolve_var(program);
                        let mut resolved_args = Vec::new();
                        for arg in args {
                            resolved_args.push(self.resolve_var(arg));
                        }

                        if let Some(AstNode { kind: AstNodeKind::Function { params, body, .. }, .. }) = self.functions.get(&resolved_program).cloned() {
                            self.variables.push(HashMap::new());
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
                            self.variables.pop();
                            self.last_exit_code = if success { 0 } else { 1 };
                            last_cmd_success = success;
                            continue;
                        }

                        let (final_program, final_args) = self.resolve_executable(&resolved_program, &resolved_args);

                        match crate::core::utils::execute_internal(&final_program, &final_args) {
                            Ok(Some(output)) => {
                                self.last_exit_code = 0;
                                last_cmd_success = true;
                                if i < nodes.len() - 1 {
                                    internal_output = Some(output);
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
                                        print!("{}", output);
                                        let _ = std::io::stdout().flush();
                                    }
                                }
                                continue;
                            }
                            Err(e) => {
                                self.last_exit_code = 1;
                                last_cmd_success = false;
                                eprintln!("{}", e);
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
                        } else if let Some(out) = internal_output.take() {
                            cmd.stdin(Stdio::piped());
                            internal_output = Some(out); // put it back to write later
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

                        if read_doc.is_some() {
                            if let Some(doc) = read_doc {
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(doc.as_bytes());
                                }
                            }
                        } else if let Some(out) = internal_output.take() {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(out.as_bytes());
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
                    let resolved_program = self.resolve_var(program);
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_var(arg));
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
                let left_val = self.capture_output(left, jobs)?;
                let right_val = self.capture_output(right, jobs)?;
                
                let success = match operator.as_str() {
                    "==" => left_val == right_val,
                    "!=" => left_val != right_val,
                    ">" | "<" | ">=" | "<=" => {
                        let l_num = left_val.parse::<f64>();
                        let r_num = right_val.parse::<f64>();
                        if let (Ok(l), Ok(r)) = (l_num, r_num) {
                            match operator.as_str() {
                                ">" => l > r,
                                "<" => l < r,
                                ">=" => l >= r,
                                "<=" => l <= r,
                                _ => false,
                            }
                        } else {
                            match operator.as_str() {
                                ">" => left_val > right_val,
                                "<" => left_val < right_val,
                                ">=" => left_val >= right_val,
                                "<=" => left_val <= right_val,
                                _ => false,
                            }
                        }
                    }
                    _ => false,
                };
                self.last_exit_code = if success { 0 } else { 1 };
                Ok(success)
            }
            AstNodeKind::Assignment { variable, value } => {
                let val = match &value.kind {
                    AstNodeKind::Array(items) => {
                        let mut arr = Vec::new();
                        for item in items {
                            arr.push(IshValue::String(self.capture_output(item, jobs)?));
                        }
                        IshValue::Array(arr)
                    }
                    AstNodeKind::Map(items) => {
                        let mut m = HashMap::new();
                        for (k, v) in items {
                            m.insert(k.clone(), IshValue::String(self.capture_output(v, jobs)?));
                        }
                        IshValue::Map(m)
                    }
                    AstNodeKind::StringLiteral(s) => {
                        IshValue::String(self.resolve_var(s))
                    }
                    _ => IshValue::String(self.capture_output(value, jobs)?),
                };
                if let Some(scope) = self.variables.last_mut() {
                    scope.insert(variable.clone(), val);
                }
                Ok(true)
            }
            AstNodeKind::If { condition, body, else_body } => {
                let success = self.execute_node_with_input(condition, "", None, jobs)?;
                let mut block_success = true;
                if success {
                    for stmt in body {
                        if self.returning { break; }
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                    }
                } else if let Some(else_stmts) = else_body {
                    for stmt in else_stmts {
                        if self.returning { break; }
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                    }
                }
                Ok(block_success)
            }
            AstNodeKind::For { variable, iterable, body } => {
                let items = self.capture_output(iterable, jobs)?;
                let mut block_success = true;
                for item in items.split_whitespace() {
                    if self.returning || self.breaking { break; }
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), IshValue::String(item.to_string()));
                    }
                    for stmt in body {
                        if self.returning || self.breaking || self.continuing { break; }
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                    }
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
                while self.execute_node_with_input(condition, "", None, jobs)? {
                    if self.returning || self.breaking { break; }
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    if self.continuing {
                        self.continuing = false;
                    }
                    if self.breaking || self.returning { break; }
                }
                if self.breaking { self.breaking = false; }
                Ok(block_success)
            }
            AstNodeKind::Break => {
                self.breaking = true;
                Ok(true)
            }
            AstNodeKind::Continue => {
                self.continuing = true;
                Ok(true)
            }
            AstNodeKind::Function { name, params: _, body: _ } => {
                self.functions.insert(name.clone(), node.clone());
                Ok(true)
            }
            AstNodeKind::Array(_) | AstNodeKind::Map(_) => {
                Ok(true)
            }
            AstNodeKind::StringLiteral(s) => {
                let resolved = self.resolve_var(s);
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
                let success = self.execute_node_with_input(inner, input, out_file, jobs)?;
                self.returning = true;
                Ok(success)
            }
            AstNodeKind::Parallel(left, right) => {
                let mut exec_left = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    functions: self.functions.clone(),
                    returning: false,
                    breaking: false,
                    continuing: false,
                };
                let mut exec_right = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    functions: self.functions.clone(),
                    returning: false,
                    breaking: false,
                    continuing: false,
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
        }
    }
}
