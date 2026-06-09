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

    pub fn execute(&mut self, ast: &AstNode, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        self.execute_node_with_input(ast, "", jobs)
    }

    fn execute_node_with_input(&mut self, node: &AstNode, input: &str, jobs: &mut JobController) -> Result<bool, IshError> 
    {
        match node {
            AstNode::Command { program, args, redirect_to, redirect_from, append_to, read_doc, merge_err: _ } => {
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
                    cmd.stdout(Stdio::inherit());
                }
                
                cmd.stderr(Stdio::inherit());

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
                    cmd.stdin(Stdio::inherit());
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

                let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });

                Ok(status.success())
            }
            AstNode::Sequential(left, right) => {
                let _s1 = self.execute_node_with_input(left, input, jobs)?;
                let s2 = self.execute_node_with_input(right, "", jobs)?;
                Ok(s2)
            }
            AstNode::AndThen(left, right) => {
                let success = self.execute_node_with_input(left, input, jobs)?;
                if success {
                    let s2 = self.execute_node_with_input(right, "", jobs)?;
                    Ok(s2)
                } else {
                    Ok(false)
                }
            }
            AstNode::OrElse(left, right) => {
                let success = self.execute_node_with_input(left, input, jobs)?;
                if !success {
                    let s2 = self.execute_node_with_input(right, "", jobs)?;
                    Ok(s2)
                } else {
                    Ok(true)
                }
            }
            AstNode::Pipeline(nodes) => {
                if nodes.is_empty() {
                    return Ok(true);
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

                        // Output redirection (from last node)
                        if let Some(AstNode::Command { redirect_to, append_to, .. }) = nodes.last() {
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
                                c.stdout(Stdio::inherit());
                            }
                        } else {
                            c.stdout(Stdio::inherit());
                        }

                        c.stderr(Stdio::inherit());

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
                                c.stdin(Stdio::inherit());
                            }
                        }

                        let mut child = c.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                        if !input.is_empty() {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(input.as_bytes());
                            }
                        }

                        let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;
                        self.last_exit_code = status.code().unwrap_or(if status.success() { 0 } else { 1 });

                        return Ok(status.success());
                    }
                }
                
                // For non-command pipelines (or non-Windows), the current implementation
                // was relying on passing string outputs. 
                // We'll return an error for now since native pipelines require stdio wiring.
                Err(IshError::ExecutionError("Complex pipelines not supported without PowerShell".into()))
            }
            AstNode::Background(inner) => {
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

                    cmd.stdout(Stdio::inherit());
                    cmd.stderr(Stdio::inherit());

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
                            println!("[Job started in background] ID: {}", job_id);
                            Ok(true)
                        }
                        Err(e) => Err(IshError::ExecutionError(e.to_string())),
                    }
                } else {
                    Err(IshError::ExecutionError("Only simple commands can run in background currently".into()))
                }
            }
            AstNode::Condition { left, operator: _operator, right } => {
                // To support conditions evaluating commands without capturing stdout,
                // we'd need subshells. For now, conditions will just evaluate the success state 
                // of the command. If left/right are not commands, they are just empty logic.
                
                let _s1 = self.execute_node_with_input(left, "", jobs)?;
                let _s2 = self.execute_node_with_input(right, "", jobs)?;
                
                // Conditions currently aren't fully robust without string capturing. 
                // If the user was comparing command outputs, it will fail here.
                Ok(false)
            }
            _ => Ok(true),
        }
    }
}
