use crate::core::ast::AstNode;
use crate::error::IshError;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::Write;
use std::collections::HashMap;
use crate::managers::job_controller::JobController;

pub struct Executor {
    pub script_args: Vec<String>,
    pub last_exit_code: i32,
    pub variables: Vec<HashMap<String, String>>,
    pub functions: HashMap<String, AstNode>,
    pub returning: bool,
}

impl Executor {
    pub fn new(script_args: Vec<String>) -> Self {
        Self { 
            script_args, 
            last_exit_code: 0,
            variables: vec![HashMap::new()],
            functions: HashMap::new(),
            returning: false,
        }
    }

    fn resolve_var(&self, s: &str) -> String {
        if s == "$LAST" {
            self.last_exit_code.to_string()
        } else if s.starts_with('$') {
            let var_name = &s[1..];
            for scope in self.variables.iter().rev() {
                if let Some(val) = scope.get(var_name) {
                    return val.clone();
                }
            }
            if let Ok(idx) = var_name.parse::<usize>() {
                if idx > 0 && idx <= self.script_args.len() {
                    return self.script_args[idx - 1].clone();
                } else {
                    return String::new();
                }
            }
            std::env::var(var_name).unwrap_or_else(|_| s.to_string())
        } else {
            s.to_string()
        }
    }

    fn capture_output(&mut self, node: &AstNode, jobs: &mut JobController) -> Result<String, IshError> {
        let temp_dir = std::env::temp_dir();
        let file_name = format!("ish_cap_{}_{}.tmp", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_path = temp_dir.join(file_name);
        let path_str = temp_path.to_str().unwrap();

        self.execute_node_with_input(node, "", Some(path_str), jobs)?;

        let out = std::fs::read_to_string(&temp_path).unwrap_or_default();
        let _ = std::fs::remove_file(&temp_path);
        Ok(out.trim().to_string())
    }

    pub fn execute(&mut self, ast: &AstNode, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        self.execute_node_with_input(ast, "", None, jobs)
    }

    fn execute_node_with_input(&mut self, node: &AstNode, input: &str, out_file: Option<&str>, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        match node {
            AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } => {
                let resolved_program = self.resolve_var(program);
                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(self.resolve_var(arg));
                }

                if let Some(AstNode::Function { params, body, .. }) = self.functions.get(&resolved_program).cloned() {
                    self.variables.push(HashMap::new());
                    
                    if let Some(scope) = self.variables.last_mut() {
                        for (i, param) in params.iter().enumerate() {
                            if i < resolved_args.len() {
                                scope.insert(param.clone(), resolved_args[i].clone());
                            }
                        }
                        for (i, arg) in resolved_args.iter().enumerate() {
                            scope.insert((i + 1).to_string(), arg.clone());
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

                let mut cmd = Command::new(&resolved_program);
                cmd.args(&resolved_args);

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

                let mut child = cmd.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

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
            AstNode::Sequential(left, right) => {
                let _s1 = self.execute_node_with_input(left, input, out_file, jobs)?;
                let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                Ok(s2)
            }
            AstNode::AndThen(left, right) => {
                let success = self.execute_node_with_input(left, input, out_file, jobs)?;
                if success {
                    let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                    Ok(s2)
                } else {
                    Ok(false)
                }
            }
            AstNode::OrElse(left, right) => {
                let success = self.execute_node_with_input(left, input, out_file, jobs)?;
                if !success {
                    let s2 = self.execute_node_with_input(right, "", out_file, jobs)?;
                    Ok(s2)
                } else {
                    Ok(true)
                }
            }
            AstNode::Pipeline(nodes) => {
                if nodes.is_empty() {
                    return Ok(true);
                }

                let mut previous_stdout: Option<std::process::ChildStdout> = None;
                let mut children = Vec::new();
                let mut last_cmd_success = true;

                for (i, node) in nodes.iter().enumerate() {
                    if let AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } = node {
                        let resolved_program = self.resolve_var(program);
                        let mut resolved_args = Vec::new();
                        for arg in args {
                            resolved_args.push(self.resolve_var(arg));
                        }

                        if let Some(AstNode::Function { params, body, .. }) = self.functions.get(&resolved_program).cloned() {
                            self.variables.push(HashMap::new());
                            if let Some(scope) = self.variables.last_mut() {
                                for (i, param) in params.iter().enumerate() {
                                    if i < resolved_args.len() {
                                        scope.insert(param.clone(), resolved_args[i].clone());
                                    }
                                }
                                for (i, arg) in resolved_args.iter().enumerate() {
                                    scope.insert((i + 1).to_string(), arg.clone());
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

                        let mut cmd = Command::new(&resolved_program);
                        cmd.args(&resolved_args);

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

                        let mut child = cmd.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                        if read_doc.is_some() {
                            if let Some(doc) = read_doc {
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(doc.as_bytes());
                                }
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
            AstNode::Background(inner) => {
                if let AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } = &**inner {
                    let resolved_program = self.resolve_var(program);
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_var(arg));
                    }
                    if self.functions.contains_key(&resolved_program) {
                        return Err(IshError::ExecutionError("Cannot run custom functions in background via AstNode::Background currently".into()));
                    }
                    let mut cmd = Command::new(&resolved_program);
                    cmd.args(&resolved_args);

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
                        Err(e) => Err(IshError::ExecutionError(e.to_string())),
                    }
                } else {
                    Err(IshError::ExecutionError("Only simple commands can run in background currently".into()))
                }
            }
            AstNode::Condition { left, operator, right } => {
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
            AstNode::Assignment { variable, value } => {
                let val = self.capture_output(value, jobs)?;
                if let Some(scope) = self.variables.last_mut() {
                    scope.insert(variable.clone(), val);
                }
                Ok(true)
            }
            AstNode::If { condition, body, else_body } => {
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
            AstNode::For { variable, iterable, body } => {
                let items = self.capture_output(iterable, jobs)?;
                let mut block_success = true;
                for item in items.split_whitespace() {
                    if self.returning { break; }
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), item.to_string());
                    }
                    for stmt in body {
                        if self.returning { break; }
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                    }
                }
                Ok(block_success)
            }
            AstNode::While { condition, body } => {
                let mut block_success = true;
                while self.execute_node_with_input(condition, "", None, jobs)? {
                    if self.returning { break; }
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning { break; }
                    }
                    if self.returning { break; }
                }
                Ok(block_success)
            }
            AstNode::Function { name, params: _, body: _ } => {
                self.functions.insert(name.clone(), node.clone());
                Ok(true)
            }
            AstNode::Return(inner) => {
                let success = self.execute_node_with_input(inner, input, out_file, jobs)?;
                self.returning = true;
                Ok(success)
            }
            AstNode::Parallel(left, right) => {
                let mut exec_left = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    functions: self.functions.clone(),
                    returning: false,
                };
                let mut exec_right = Executor {
                    script_args: self.script_args.clone(),
                    last_exit_code: 0,
                    variables: self.variables.clone(),
                    functions: self.functions.clone(),
                    returning: false,
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
