use crate::core::ast::AstNode;
use crate::error::IshError;
use std::process::{Command, Stdio};
use std::fs::File;
use std::io::{Read, Write};

pub struct Executor {}

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn execute(&mut self, ast: &AstNode) -> Result<(bool, String), IshError> {
        self.execute_node_with_input(ast, "")
    }

    fn execute_node_with_input(&mut self, node: &AstNode, input: &str) -> Result<(bool, String), IshError> {
        match node {
            AstNode::Command { program, args, redirect_to, redirect_from } => {
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("powershell");
                    c.arg("-NoProfile").arg("-Command");
                    
                    let mut full_cmd = program.clone();
                    for arg in args {
                        full_cmd.push(' ');
                        full_cmd.push_str(arg);
                    }
                    c.arg(full_cmd);
                    c
                } else {
                    let mut c = Command::new(program);
                    c.args(args);
                    c
                };

                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());

                if let Some(path) = redirect_to {
                    let file = File::create(path)?;
                    cmd.stdout(Stdio::from(file));
                }
                
                if let Some(path) = redirect_from {
                    let file = File::open(path)?;
                    cmd.stdin(Stdio::from(file));
                } else if !input.is_empty() {
                    cmd.stdin(Stdio::piped());
                } else {
                    cmd.stdin(Stdio::inherit());
                }

                let mut child = cmd.spawn().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                if !input.is_empty() {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(input.as_bytes());
                    }
                }

                let status = child.wait().map_err(|e| IshError::ExecutionError(e.to_string()))?;

                let mut output = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    let _ = stdout.read_to_string(&mut output);
                }
                if let Some(mut stderr) = child.stderr.take() {
                    let _ = stderr.read_to_string(&mut output);
                }

                Ok((status.success(), output))
            }
            AstNode::Sequential(left, right) => {
                let (_s1, mut out1) = self.execute_node_with_input(left, input)?;
                let (s2, out2) = self.execute_node_with_input(right, "")?;
                out1.push_str(&out2);
                Ok((s2, out1))
            }
            AstNode::AndThen(left, right) => {
                let (success, mut out) = self.execute_node_with_input(left, input)?;
                if success {
                    let (s2, out2) = self.execute_node_with_input(right, "")?;
                    out.push_str(&out2);
                    Ok((s2, out))
                } else {
                    Ok((false, out))
                }
            }
            AstNode::OrElse(left, right) => {
                let (success, mut out) = self.execute_node_with_input(left, input)?;
                if !success {
                    let (s2, out2) = self.execute_node_with_input(right, "")?;
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
                
                let mut current_input = input.to_string();
                let mut last_success = true;

                for node in nodes {
                    let (success, out) = self.execute_node_with_input(node, &current_input)?;
                    last_success = success;
                    current_input = out;
                }
                Ok((last_success, current_input))
            }
            AstNode::Background(_inner) => {
                Ok((true, "[Job started in background]\n".to_string()))
            }
            _ => Ok((true, String::new())),
        }
    }
}
