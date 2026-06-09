use crate::core::ast::AstNode;
use crate::error::IshError;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::Write;
use crate::managers::job_controller::JobController;

pub struct Executor {
    pub script_args: Vec<String>,
    pub last_exit_code: i32,
}

impl Executor {
    pub fn new(script_args: Vec<String>) -> Self {
        Self { script_args, last_exit_code: 0 }
    }

    fn resolve_var(&self, s: &str) -> String {
        if s == "$LAST" {
            self.last_exit_code.to_string()
        } else if s.starts_with('$') {
            if let Ok(idx) = s[1..].parse::<usize>() {
                if idx > 0 && idx <= self.script_args.len() {
                    self.script_args[idx - 1].clone()
                } else {
                    String::new()
                }
            } else {
                s.to_string() // unresolved or env var
            }
        } else {
            s.to_string()
        }
    }

    pub fn execute<F>(&mut self, ast: &AstNode, jobs: &mut JobController, pump_callback: &mut F) -> Result<(bool, String), IshError> 
    where F: FnMut(&mut std::process::Child, bool) -> Result<String, IshError>
    {
        self.execute_node_with_input(ast, "", jobs, pump_callback)
    }

    fn execute_node_with_input<F>(&mut self, node: &AstNode, input: &str, jobs: &mut JobController, pump_callback: &mut F) -> Result<(bool, String), IshError> 
    where F: FnMut(&mut std::process::Child, bool) -> Result<String, IshError>
    {
        match node {
            AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err } => {
                let resolved_program = self.resolve_var(program);
                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(self.resolve_var(arg));
                }

                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-NoProfile").arg("-Command");
                    
                    let mut full_cmd = resolved_program.clone();
                    for arg in &resolved_args {
                        full_cmd.push(' ');
                        full_cmd.push_str(arg);
                    }
                    c.arg(full_cmd);
                    c
                } else {
                    let mut c = Command::new(&resolved_program);
                    c.args(&resolved_args);
                    c
                };

                if let Some(path) = redirect_to {
                    if path == "DevNull" {
                        cmd.stdout(Stdio::null());
                    } else {
                        let file = File::create(path)?;
                        cmd.stdout(Stdio::from(file));
                    }
                } else if let Some(path) = append_to {
                    if path == "DevNull" {
                        cmd.stdout(Stdio::null());
                    } else {
                        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                        cmd.stdout(Stdio::from(file));
                    }
                } else {
                    cmd.stdout(Stdio::piped());
                }
                
                cmd.stderr(Stdio::piped());

                if let Some(path) = redirect_from {
                    if path == "DevNull" {
                        cmd.stdin(Stdio::null());
                    } else {
                        let file = File::open(path)?;
                        cmd.stdin(Stdio::from(file));
                    }
                } else if let Some(_) = read_doc {
                    cmd.stdin(Stdio::piped());
                } else if !input.is_empty() {
                    cmd.stdin(Stdio::piped());
                } else {
                    cmd.stdin(Stdio::piped());
                }

                let mut child = cmd.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                if let Some(_) = read_doc {
                    if !input.is_empty() {
                        if let Some(mut stdin) = child.stdin.take() {
                            let _ = stdin.write_all(input.as_bytes());
                        }
                    }
                } else if !input.is_empty() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(input.as_bytes());
                    }
                }

                let output = pump_callback(&mut child, *merge_err)?;

                let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });

                Ok((status.success(), output))
            }
            AstNode::Sequential(left, right) => {
                let (_s1, mut out1) = self.execute_node_with_input(left, input, jobs, pump_callback)?;
                let (s2, out2) = self.execute_node_with_input(right, "", jobs, pump_callback)?;
                out1.push_str(&out2);
                Ok((s2, out1))
            }
            AstNode::AndThen(left, right) => {
                let (success, mut out) = self.execute_node_with_input(left, input, jobs, pump_callback)?;
                if success {
                    let (s2, out2) = self.execute_node_with_input(right, "", jobs, pump_callback)?;
                    out.push_str(&out2);
                    Ok((s2, out))
                } else {
                    Ok((false, out))
                }
            }
            AstNode::OrElse(left, right) => {
                let (success, mut out) = self.execute_node_with_input(left, input, jobs, pump_callback)?;
                if !success {
                    let (s2, out2) = self.execute_node_with_input(right, "", jobs, pump_callback)?;
                    out.push_str(&out2);
                    Ok((s2, out))
                } else {
                    Ok((true, out))
                }
            }
            AstNode::Pipeline(nodes) => {
                if nodes.is_empty() {
                    return Ok((true, String::new()));
                }

                if cfg!(target_os = "windows") {
                    let mut all_commands = true;
                    for node in nodes {
                        if !matches!(node, AstNode::Command { .. }) {
                            all_commands = false;
                            break;
                        }
                    }

                    if all_commands {
                        let mut full_ps_cmd = String::new();
                        for (i, node) in nodes.iter().enumerate() {
                            if let AstNode::Command { program, args, .. } = node {
                                full_ps_cmd.push_str(&self.resolve_var(program));
                                for arg in args {
                                    full_ps_cmd.push(' ');
                                    full_ps_cmd.push_str(&self.resolve_var(arg));
                                }
                                if i < nodes.len() - 1 {
                                    full_ps_cmd.push_str(" | ");
                                }
                            }
                        }

                        let mut c = Command::new("powershell");
                        c.arg("-NoProfile").arg("-Command").arg(full_ps_cmd);

                        // Handle redirections
                        let mut merge_error = false;

                        // Output redirection (from last node)
                        if let Some(AstNode::Command { redirect_to, append_to, merge_err, .. }) = nodes.last() {
                            merge_error = *merge_err;
                            if let Some(path) = redirect_to {
                                if path == "DevNull" {
                                    c.stdout(Stdio::null());
                                } else {
                                    let file = File::create(path)?;
                                    c.stdout(Stdio::from(file));
                                }
                            } else if let Some(path) = append_to {
                                if path == "DevNull" {
                                    c.stdout(Stdio::null());
                                } else {
                                    let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                                    c.stdout(Stdio::from(file));
                                }
                            } else {
                                c.stdout(Stdio::piped());
                            }
                        } else {
                            c.stdout(Stdio::piped());
                        }

                        c.stderr(Stdio::piped());

                        // Input redirection (from first node)
                        if let Some(AstNode::Command { redirect_from, read_doc, .. }) = nodes.first() {
                            if let Some(path) = redirect_from {
                                if path == "DevNull" {
                                    c.stdin(Stdio::null());
                                } else {
                                    let file = File::open(path)?;
                                    c.stdin(Stdio::from(file));
                                }
                            } else if read_doc.is_some() {
                                c.stdin(Stdio::piped());
                            } else if !input.is_empty() {
                                c.stdin(Stdio::piped());
                            } else {
                                c.stdin(Stdio::inherit());
                            }
                        } else {
                            if !input.is_empty() {
                                c.stdin(Stdio::piped());
                            } else {
                                c.stdin(Stdio::piped());
                            }
                        }

                        let mut child = c.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                        if !input.is_empty() {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(input.as_bytes());
                            }
                        }

                        let output = pump_callback(&mut child, merge_error)?;

                        let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                        self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });

                        return Ok((status.success(), output));
                    }
                }
                
                let mut current_input = input.to_string();
                let mut last_success = true;

                for node in nodes {
                    let (success, out) = self.execute_node_with_input(node, &current_input, jobs, pump_callback)?;
                    last_success = success;
                    current_input = out;
                }
                Ok((last_success, current_input))
            }
            AstNode::Background(inner) => {
                // If the inner is a command, we spawn it without waiting.
                if let AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc: _, merge_err: _ } = &**inner {
                    let resolved_program = self.resolve_var(program);
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_var(arg));
                    }
                    let mut cmd = if cfg!(target_os = "windows") {
                        let mut c = Command::new("powershell");
                        c.arg("-NoProfile").arg("-Command");
                        
                        let mut full_cmd = resolved_program.clone();
                        for arg in &resolved_args {
                            full_cmd.push(' ');
                            full_cmd.push_str(arg);
                        }
                        c.arg(full_cmd);
                        c
                    } else {
                        let mut c = Command::new(&resolved_program);
                        c.args(&resolved_args);
                        c
                    };

                    cmd.stdout(Stdio::piped());
                    cmd.stderr(Stdio::piped());

                    if let Some(path) = redirect_to {
                        if path == "DevNull" {
                            cmd.stdout(Stdio::null());
                        } else {
                            let file = File::create(path)?;
                            cmd.stdout(Stdio::from(file));
                        }
                    } else if let Some(path) = append_to {
                        if path == "DevNull" {
                            cmd.stdout(Stdio::null());
                        } else {
                            let file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                            cmd.stdout(Stdio::from(file));
                        }
                    }
                    if let Some(path) = redirect_from {
                        if path == "DevNull" {
                            cmd.stdin(Stdio::null());
                        } else {
                            let file = File::open(path)?;
                            cmd.stdin(Stdio::from(file));
                        }
                    }

                    match cmd.spawn() {
                        Ok(child) => {
                            let job_id = jobs.add_job(child);
                            Ok((true, format!("[Job started in background] ID: {}\n", job_id)))
                        }
                        Err(e) => Err(IshError::ExecutionError(e.to_string())),
                    }
                } else {
                    Err(IshError::ExecutionError("Only simple commands can run in background currently".into()))
                }
            }
            AstNode::Condition { left, operator, right } => {
                let (_, mut left_val) = self.execute_node_with_input(left, "", jobs, pump_callback)?;
                let (_, mut right_val) = self.execute_node_with_input(right, "", jobs, pump_callback)?;
                left_val = left_val.trim().to_string();
                right_val = right_val.trim().to_string();

                let is_true = if let (Ok(l), Ok(r)) = (left_val.parse::<f64>(), right_val.parse::<f64>()) {
                    match operator.as_str() {
                        "==" => l == r,
                        "!=" => l != r,
                        ">" => l > r,
                        "<" => l < r,
                        ">=" => l >= r,
                        "<=" => l <= r,
                        _ => false,
                    }
                } else {
                    match operator.as_str() {
                        "==" => left_val == right_val,
                        "!=" => left_val != right_val,
                        ">" => left_val > right_val,
                        "<" => left_val < right_val,
                        ">=" => left_val >= right_val,
                        "<=" => left_val <= right_val,
                        _ => false,
                    }
                };

                Ok((is_true, String::new()))
            }
            _ => Ok((true, String::new())),
        }
    }
}
