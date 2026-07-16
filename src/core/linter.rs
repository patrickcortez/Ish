use crate::core::ast::{AstNode, AstNodeKind};
use crate::error::IshError;
use std::collections::{HashMap, HashSet};

pub struct Linter {
    defined_vars: Vec<HashSet<String>>,
    function_bases: Vec<usize>,
    defined_functions: HashMap<String, usize>, // name -> param count
    in_loop_depth: usize,
    current_class_fields: HashSet<String>,
}

impl Linter {
    pub fn new() -> Self {
        Self {
            defined_vars: vec![HashSet::new()],
            function_bases: vec![0],
            defined_functions: HashMap::new(),
            in_loop_depth: 0,
            current_class_fields: HashSet::new(),
        }
    }

    pub fn lint(&mut self, ast: &AstNode) -> Result<(), IshError> {
        self.check_node(ast)
    }

    fn is_var_defined(&self, var_name: &str) -> bool {
        if var_name.is_empty() || var_name == "?" || var_name == "LAST" || var_name == "$" || var_name == "!" || var_name == "#" || var_name == "@" || var_name == "0" {
            return true;
        }
        
        if self.current_class_fields.contains(var_name) {
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
            AstNodeKind::BinaryOp { left, right, .. } => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNodeKind::IndexAccess { object, index } => {
                self.check_node(object)?;
                self.check_node(index)?;
            }
            AstNodeKind::UnaryOp { operand, .. } => {
                self.check_node(operand)?;
            }
            AstNodeKind::TernaryOp { condition, true_value, false_value } => {
                self.check_node(condition)?;
                self.check_node(true_value)?;
                self.check_node(false_value)?;
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
            AstNodeKind::Assignment { variable, index: _, value, is_declaration, .. } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Assignment missing variable name", node.line, node.column)));
                }
                self.check_node(value)?;
                if *is_declaration {
                    if let Some(scope) = self.defined_vars.last_mut() {
                        scope.insert(variable.clone());
                    }
                } else {
                    if !variable.contains('.') && !self.is_var_defined(variable) {
                        return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: Variable '{}' is assigned before being declared", node.line, node.column, variable)));
                    }
                }
            }
            AstNodeKind::FieldDecl { default_value, .. } => {
                if let Some(val) = default_value {
                    self.check_node(val)?;
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

            AstNodeKind::CharLiteral(_) => {}

            AstNodeKind::Variable(_) => {}
            AstNodeKind::Return(inner) => {
                fn check_invalid_return(n: &AstNode) -> Option<String> {
                    match &n.kind {
                        AstNodeKind::Assignment { value, .. } => check_invalid_return(value),
                        AstNodeKind::BinaryOp { left, right, .. } => {
                            if let Some(err) = check_invalid_return(left) {
                                return Some(err);
                            }
                            check_invalid_return(right)
                        }
                        _ => None,
                    }
                }
                
                if let Some(invalid_cmd) = check_invalid_return(inner) {
                    return Err(IshError::ParseError(format!("Linter Error at Line {}, Column {}: cannot return command '{}'. Returns can only return values or variables.", node.line, node.column, invalid_cmd)));
                }

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
            AstNodeKind::NamespaceDecl { body, .. } => {
                for stmt in body {
                    self.check_node(stmt)?;
                }
            }
            AstNodeKind::ClassDecl { methods, fields, constructor, destructor, base_class: _, .. } => {
                let prev_fields = self.current_class_fields.clone();
                self.current_class_fields.clear();
                for field in fields {
                    if let AstNodeKind::FieldDecl { name, .. } = &field.kind {
                        self.current_class_fields.insert(name.clone());
                    }
                }

                if let Some(c) = constructor {
                    self.defined_vars.push(std::collections::HashSet::new());
                    self.function_bases.push(self.defined_vars.len() - 1);
                    if let Some(scope) = self.defined_vars.last_mut() {
                        for param in &c.1 {
                            scope.insert(param.name.clone());
                        }
                    }
                    for stmt in &c.2 {
                        self.check_node(stmt)?;
                    }
                    self.function_bases.pop();
                    self.defined_vars.pop();
                }
                if let Some(d) = destructor {
                    self.defined_vars.push(std::collections::HashSet::new());
                    self.function_bases.push(self.defined_vars.len() - 1);
                    for stmt in d {
                        self.check_node(stmt)?;
                    }
                    self.function_bases.pop();
                    self.defined_vars.pop();
                }
                for method in methods {
                    self.check_node(method)?;
                }
                for field in fields {
                    self.check_node(field)?;
                }
                
                self.current_class_fields = prev_fields;
            }
            AstNodeKind::StructDecl { fields, constructor, destructor, .. } => {
                let prev_fields = self.current_class_fields.clone();
                self.current_class_fields.clear();
                for field in fields {
                    if let AstNodeKind::FieldDecl { name, .. } = &field.kind {
                        self.current_class_fields.insert(name.clone());
                    }
                }

                if let Some(c) = constructor {
                    self.defined_vars.push(std::collections::HashSet::new());
                    self.function_bases.push(self.defined_vars.len() - 1);
                    if let Some(scope) = self.defined_vars.last_mut() {
                        for param in &c.1 {
                            scope.insert(param.name.clone());
                        }
                    }
                    for stmt in &c.2 {
                        self.check_node(stmt)?;
                    }
                    self.function_bases.pop();
                    self.defined_vars.pop();
                }
                if let Some(d) = destructor {
                    self.defined_vars.push(std::collections::HashSet::new());
                    self.function_bases.push(self.defined_vars.len() - 1);
                    for stmt in d {
                        self.check_node(stmt)?;
                    }
                    self.function_bases.pop();
                    self.defined_vars.pop();
                }
                for field in fields {
                    self.check_node(field)?;
                }
                
                self.current_class_fields = prev_fields;
            }
            AstNodeKind::ObjectInstantiation { args, initializer, .. } => {
                for arg in args {
                    self.check_node(arg)?;
                }
                if let Some(init) = initializer {
                    for node in init {
                        self.check_node(node)?;
                    }
                }
            }
            AstNodeKind::Switch { expression, cases, default_case } => {
                self.check_node(expression)?;
                for (case_expr, body) in cases {
                    self.check_node(case_expr)?;
                    for stmt in body {
                        self.check_node(stmt)?;
                    }
                }
                if let Some(body) = default_case {
                    for stmt in body {
                        self.check_node(stmt)?;
                    }
                }
            }
            AstNodeKind::InterpolatedString(nodes) => {
                for node in nodes {
                    self.check_node(node)?;
                }
            }
            AstNodeKind::KeyValuePair { key, value } => {
                self.check_node(key)?;
                self.check_node(value)?;
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