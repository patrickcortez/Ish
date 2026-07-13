use crate::core::ast::{AstNode, AstNodeKind};
use crate::error::IshError;
use std::collections::{HashMap, HashSet};

pub struct Linter {
    defined_vars: Vec<HashSet<String>>,
    function_bases: Vec<usize>,
    defined_functions: HashMap<String, usize>, // name -> param count
    in_loop_depth: usize,
}

impl Linter {
    pub fn new() -> Self {
        Self {
            defined_vars: vec![HashSet::new()],
            function_bases: vec![0],
            defined_functions: HashMap::new(),
            in_loop_depth: 0,
        }
    }

    pub fn lint(&mut self, ast: &AstNode) -> Result<(), IshError> {
        self.check_node(ast)
    }

    fn is_var_defined(&self, var_name: &str) -> bool {
        if var_name.is_empty() || var_name == "?" || var_name == "LAST" || var_name == "$" || var_name == "!" || var_name == "#" || var_name == "@" || var_name == "0" {
            return true;
        }
        
        let base_idx = *self.function_bases.last().unwrap_or(&0);
        let mut search_scopes = vec![];
        for i in (base_idx..self.defined_vars.len()).rev() {
            search_scopes.push(i);
        }
        if base_idx != 0 {
            search_scopes.push(0);
        }
        
        for &idx in &search_scopes {
            let scope = &self.defined_vars[idx];
            if scope.contains(var_name) {
                return true;
            }
        }
        false
    }

    fn check_variables_in_string(&self, s: &str, line: usize, col: usize) -> Result<(), IshError> {
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '$' {
                if chars.peek() == Some(&'(') {
                    continue; // Skip $(
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
                if !var_name.is_empty() && !self.is_var_defined(&var_name) {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Variable '{}' is not defined in this scope", line, col, var_name)));
                }
            }
        }
        Ok(())
    }

    fn check_node(&mut self, node: &AstNode) -> Result<(), IshError> {
        match &node.kind {
            AstNodeKind::Command { program, args, redirect_to, redirect_from, .. } => {
                if program.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Empty command detected", node.line, node.column)));
                }

                self.check_variables_in_string(program, node.line, node.column)?;
                for arg in args {
                    self.check_variables_in_string(arg, node.line, node.column)?;
                }

                if !program.starts_with('$') {
                    if let Some(expected_args) = self.defined_functions.get(program) {
                        if args.len() != *expected_args {
                            return Err(IshError::ParseError(format!(
                                "Linter Error at Line {}, Column {}: Function '{}' expects {} arguments, but got {}",
                                node.line, node.column, program, expected_args, args.len()
                            )));
                        }
                    }
                }

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
            AstNodeKind::BinaryOp { left, right, .. } => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNodeKind::UnaryOp { operand, .. } => {
                self.check_node(operand)?;
            }
            AstNodeKind::TernaryOp { condition, true_value, false_value } => {
                self.check_node(condition)?;
                self.check_node(true_value)?;
                self.check_node(false_value)?;
            }

            AstNodeKind::Background(inner) => {
                self.check_node(inner)?;
            }
            AstNodeKind::If { condition, body, else_body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'if' statement has an empty body", node.line, node.column)));
                }
                
                self.defined_vars.push(HashSet::new());
                for n in body {
                    self.check_node(n)?;
                }
                self.defined_vars.pop();

                if let Some(eb) = else_body {
                    if eb.is_empty() {
                        return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'else' statement has an empty body", node.line, node.column)));
                    }
                    self.defined_vars.push(HashSet::new());
                    for n in eb {
                        self.check_node(n)?;
                    }
                    self.defined_vars.pop();
                }
            }
            AstNodeKind::TryCatch { try_body, error_var, catch_body } => {
                if try_body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'try' block is empty", node.line, node.column)));
                }
                self.defined_vars.push(HashSet::new());
                for n in try_body {
                    self.check_node(n)?;
                }
                self.defined_vars.pop();

                self.defined_vars.push(HashSet::new());
                if let Some(scope) = self.defined_vars.last_mut() {
                    scope.insert(error_var.clone());
                }
                for n in catch_body {
                    self.check_node(n)?;
                }
                self.defined_vars.pop();
            }
            AstNodeKind::While { condition, body } => {
                self.check_node(condition)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'while' loop has an empty body", node.line, node.column)));
                }
                self.in_loop_depth += 1;
                self.defined_vars.push(HashSet::new());
                for n in body {
                    self.check_node(n)?;
                }
                self.defined_vars.pop();
                self.in_loop_depth -= 1;
            }
            AstNodeKind::For { variable, iterable, body } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'for' loop missing variable name", node.line, node.column)));
                }
                self.check_node(iterable)?;
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'for' loop has an empty body", node.line, node.column)));
                }
                self.in_loop_depth += 1;
                self.defined_vars.push(HashSet::new());
                if let Some(scope) = self.defined_vars.last_mut() {
                    scope.insert(variable.clone());
                }
                for n in body {
                    self.check_node(n)?;
                }
                self.defined_vars.pop();
                self.in_loop_depth -= 1;
            }
            AstNodeKind::Assignment { variable, index: _, value, is_declaration } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Assignment missing variable name", node.line, node.column)));
                }
                self.check_node(value)?;
                if *is_declaration {
                    if let Some(scope) = self.defined_vars.last_mut() {
                        scope.insert(variable.clone());
                    }
                } else {
                    if !self.is_var_defined(variable) {
                        return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Variable '{}' is assigned before being declared", node.line, node.column, variable)));
                    }
                }
            }
            AstNodeKind::Function { name, params, body, .. } => {
                if name.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Function defined without a name", node.line, node.column)));
                }
                if body.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Function '{}' has an empty body", node.line, node.column, name)));
                }
                self.defined_functions.insert(name.clone(), params.len());
                
                for param in params {
                    if let Some(def) = &param.default_value {
                        self.check_node(def)?;
                    }
                }
                
                self.defined_vars.push(HashSet::new());
                self.function_bases.push(self.defined_vars.len() - 1);
                
                if let Some(scope) = self.defined_vars.last_mut() {
                    for param in params {
                        scope.insert(param.name.clone());
                    }
                }
                
                for stmt in body {
                    self.check_node(stmt)?;
                }
                
                self.function_bases.pop();
                self.defined_vars.pop();
            }
            AstNodeKind::Break | AstNodeKind::Continue => {
                if self.in_loop_depth == 0 {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: 'break' or 'continue' used outside of a loop", node.line, node.column)));
                }
            }
            AstNodeKind::StringLiteral(s) => {
                self.check_variables_in_string(s, node.line, node.column)?;
            }
            AstNodeKind::Return(inner) => {
                fn check_invalid_return(n: &AstNode) -> Option<String> {
                    match &n.kind {
                        AstNodeKind::Command { program, .. } => {
                            let invalid_cmds = ["out", "show", "cd", "export", "alias", "unalias", "kill", "fg", "jobs", "declare", "let"];
                            if invalid_cmds.contains(&program.as_str()) {
                                return Some(program.clone());
                            }
                        }
                        AstNodeKind::Pipeline(commands) => {
                            if let Some(last) = commands.last() {
                                return check_invalid_return(last);
                            }
                        }
                        AstNodeKind::Subshell(inner_sub) => {
                            return check_invalid_return(inner_sub);
                        }
                        _ => {}
                    }
                    None
                }
                
                if let Some(invalid_cmd) = check_invalid_return(inner) {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: cannot return command '{}'. Returns can only return values or variables.", node.line, node.column, invalid_cmd)));
                }

                self.check_node(inner)?;
            }
            AstNodeKind::Subshell(inner) => {
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
            // ---- OOP Nodes ----
            AstNodeKind::NamespaceDecl { body, .. } => {
                for stmt in body {
                    self.check_node(stmt)?;
                }
            }
            AstNodeKind::ClassDecl { methods, fields, constructor, destructor, .. } => {
                if let Some(c) = constructor {
                    for stmt in c {
                        self.check_node(stmt)?;
                    }
                }
                if let Some(d) = destructor {
                    for stmt in d {
                        self.check_node(stmt)?;
                    }
                }
                for method in methods {
                    self.check_node(method)?;
                }
                for field in fields {
                    self.check_node(field)?;
                }
            }
            AstNodeKind::StructDecl { fields, .. } => {
                for field in fields {
                    self.check_node(field)?;
                }
            }
            AstNodeKind::ObjectInstantiation { args, .. } => {
                for arg in args {
                    self.check_node(arg)?;
                }
            }
            AstNodeKind::MethodCall { object, args, .. } => {
                self.check_node(object)?;
                for arg in args {
                    self.check_node(arg)?;
                }
            }
            AstNodeKind::PropertyAccess { object, .. } => {
                self.check_node(object)?;
            }
            AstNodeKind::EnumDecl { .. } => {}
            AstNodeKind::WithImport { .. } => {}
        }
        Ok(())
    }
}
