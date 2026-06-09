use crate::core::ast::AstNode;
use crate::error::IshError;
use std::collections::{HashMap, HashSet};

pub struct Linter {
    defined_vars: HashSet<String>,
    defined_functions: HashMap<String, usize>, // name -> param count
    in_loop_depth: usize,
}

impl Linter {
    pub fn new() -> Self {
        Self {
            defined_vars: HashSet::new(),
            defined_functions: HashMap::new(),
            in_loop_depth: 0,
        }
    }

    pub fn lint(&mut self, ast: &AstNode) -> Result<(), IshError> {
        self.check_node(ast)
    }

    fn check_node(&mut self, node: &AstNode) -> Result<(), IshError> {
        match node {
            AstNode::Command { program, args, redirect_to, redirect_from, .. } => {
                if program.is_empty() {
                    return Err(IshError::ParseError("Empty command detected".to_string()));
                }

                // If program is a variable, make sure it is defined
                if program.starts_with('$') {
                    let _var_name = &program[1..];
                    // We can't strictly enforce because environment vars exist, but we could warn.
                    // For now, shell allows undefined vars to resolve to empty strings.
                } else {
                    // Check if program is a function, if so check arity.
                    // If not in functions, we assume it's an external OS command.
                    if let Some(expected_args) = self.defined_functions.get(program) {
                        if args.len() != *expected_args && *expected_args > 0 {
                            // Only strictly check if params were named. If params is 0, they might use $1, $2 dynamically.
                            if !args.is_empty() && *expected_args > 0 && args.len() != *expected_args {
                                // Just a soft warning or notice normally, but we are a strict linter!
                                // Wait, bash allows variadic args. Let's allow it but we could add strict modes later.
                            }
                        }
                    }
                }

                // Check invalid redirection combinations
                if redirect_to.is_some() && redirect_to == redirect_from {
                    return Err(IshError::ParseError(format!(
                        "Linter Error: Cannot redirect to and from the same file '{}'",
                        redirect_to.as_ref().unwrap()
                    )));
                }
            }
            AstNode::Pipeline(nodes) => {
                if nodes.len() < 2 {
                    return Err(IshError::ParseError("Linter Error: Pipeline requires at least two commands".to_string()));
                }
                for n in nodes {
                    self.check_node(n)?;
                }
            }
            AstNode::Sequential(left, right)
            | AstNode::AndThen(left, right)
            | AstNode::OrElse(left, right)
            | AstNode::Parallel(left, right) => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNode::Condition { left, right, .. } => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNode::Background(inner) => {
                self.check_node(inner)?;
            }
            AstNode::If { condition, body, else_body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError("Linter Error: 'if' statement has an empty body".to_string()));
                }
                for n in body {
                    self.check_node(n)?;
                }
                if let Some(eb) = else_body {
                    if eb.is_empty() {
                        return Err(IshError::ParseError("Linter Error: 'else' statement has an empty body".to_string()));
                    }
                    for n in eb {
                        self.check_node(n)?;
                    }
                }
            }
            AstNode::While { condition, body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError("Linter Error: 'while' loop has an empty body. Infinite spin risk!".to_string()));
                }
                self.in_loop_depth += 1;
                for n in body {
                    self.check_node(n)?;
                }
                self.in_loop_depth -= 1;
            }
            AstNode::For { variable, iterable, body } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError("Linter Error: 'for' loop missing variable name".to_string()));
                }
                self.defined_vars.insert(variable.clone());
                self.check_node(iterable)?;
                if body.is_empty() {
                    return Err(IshError::ParseError("Linter Error: 'for' loop has an empty body".to_string()));
                }
                self.in_loop_depth += 1;
                for n in body {
                    self.check_node(n)?;
                }
                self.in_loop_depth -= 1;
            }
            AstNode::Assignment { variable, value } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError("Linter Error: Assignment missing variable name".to_string()));
                }
                self.check_node(value)?;
                self.defined_vars.insert(variable.clone());
            }
            AstNode::Function { name, params, body } => {
                if name.is_empty() {
                    return Err(IshError::ParseError("Linter Error: Function defined without a name".to_string()));
                }
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error: Function '{}' has an empty body", name)));
                }
                self.defined_functions.insert(name.clone(), params.len());
                
                // Add params to defined vars temporarily
                let mut added_params = Vec::new();
                for p in params {
                    if !self.defined_vars.contains(p) {
                        self.defined_vars.insert(p.clone());
                        added_params.push(p.clone());
                    }
                }

                for n in body {
                    self.check_node(n)?;
                }

                // Remove params
                for p in added_params {
                    self.defined_vars.remove(&p);
                }
            }
            AstNode::Return(inner) => {
                self.check_node(inner)?;
            }
        }
        Ok(())
    }
}
