use crate::core::ast::{AstNode, AstNodeKind};
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
        match &node.kind {
            AstNodeKind::Command { program, args, redirect_to, redirect_from, .. } => {
                if program.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Empty command detected", node.line, node.column)));
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
                        "Linter Error at Line {}, Column {}: Cannot redirect to and from the same file '{}'",
                        node.line, node.column,
                        redirect_to.as_ref().unwrap()
                    )));
                }
            }
            AstNodeKind::Pipeline(nodes) => {
                if nodes.len() < 2 {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Pipeline requires at least two commands", node.line, node.column)));
                }
                for n in nodes {
                    self.check_node(n)?;
                }
            }
            AstNodeKind::Sequential(left, right)
            | AstNodeKind::AndThen(left, right)
            | AstNodeKind::OrElse(left, right)
            | AstNodeKind::Parallel(left, right) => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNodeKind::Condition { left, right, .. } => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNodeKind::BinaryOperation { left, right, .. } => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNodeKind::Background(inner) => {
                self.check_node(inner)?;
            }
            AstNodeKind::If { condition, body, else_body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'if' statement has an empty body", node.line, node.column)));
                }
                for n in body {
                    self.check_node(n)?;
                }
                if let Some(eb) = else_body {
                    if eb.is_empty() {
                        return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'else' statement has an empty body", node.line, node.column)));
                    }
                    for n in eb {
                        self.check_node(n)?;
                    }
                }
            }
            AstNodeKind::TryCatch { try_body, error_var, catch_body } => {
                if try_body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'try' block is empty", node.line, node.column)));
                }
                for n in try_body {
                    self.check_node(n)?;
                }
                self.defined_vars.insert(error_var.clone());
                for n in catch_body {
                    self.check_node(n)?;
                }
            }
            AstNodeKind::While { condition, body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'while' loop has an empty body. Infinite spin risk!", node.line, node.column)));
                }
                self.in_loop_depth += 1;
                for n in body {
                    self.check_node(n)?;
                }
                self.in_loop_depth -= 1;
            }
            AstNodeKind::For { variable, iterable, body } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'for' loop missing variable name", node.line, node.column)));
                }
                self.defined_vars.insert(variable.clone());
                self.check_node(iterable)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'for' loop has an empty body", node.line, node.column)));
                }
                self.in_loop_depth += 1;
                for n in body {
                    self.check_node(n)?;
                }
                self.in_loop_depth -= 1;
            }
            AstNodeKind::Assignment { variable, value, is_declaration: _ } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Assignment missing variable name", node.line, node.column)));
                }
                self.check_node(value)?;
                self.defined_vars.insert(variable.clone());
            }
            AstNodeKind::Function { name, params, body } => {
                if name.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Function defined without a name", node.line, node.column)));
                }
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Function '{}' has an empty body", node.line, node.column, name)));
                }
                self.defined_functions.insert(name.clone(), params.len());
                let prev_vars = self.defined_vars.clone();
                for param in params {
                    self.defined_vars.insert(param.clone());
                }
                for stmt in body {
                    self.check_node(stmt)?;
                }
                self.defined_vars = prev_vars;
            }
            AstNodeKind::Break | AstNodeKind::Continue | AstNodeKind::StringLiteral(_) => {
                if self.in_loop_depth == 0 && matches!(node.kind, AstNodeKind::Break | AstNodeKind::Continue) {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'break' or 'continue' used outside of a loop", node.line, node.column)));
                }
            }
            AstNodeKind::Return(inner) => {
                self.check_node(inner)?;
            }
            AstNodeKind::Array(items) => {
                for item in items {
                    self.check_node(item)?;
                }
            }
            AstNodeKind::Map(items) => {
                for (_, val) in items {
                    self.check_node(val)?;
                }
            }
        }
        Ok(())
    }
}
