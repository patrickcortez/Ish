use crate::core::ast::{AstNode, AstNodeKind, IshValue};
use crate::error::IshError;
use std::io::Write;
use std::collections::HashMap;
use crate::core::stdlib::{StdlibProvider, IshStr, IshFS, IshTime, IshNet, IshOS};
use crate::core::registry::Registry;
use crate::managers::job_controller::JobController;
use crate::core::gobbler::{Gobbler, HeapObject};

pub struct Executor {
    pub script_args: Vec<String>,
    pub last_exit_code: i32,
    pub variables: Vec<HashMap<String, IshValue>>,
    pub static_variables: HashMap<String, IshValue>,
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
    pub gobbler: Gobbler,
    pub is_evaluating: bool,
}

impl Executor {
    pub fn new(script_args: Vec<String>) -> Self {
        Self { 
            script_args, 
            last_exit_code: 0,
            variables: vec![HashMap::new()],
            static_variables: HashMap::new(),
            function_bases: vec![0],
            return_value: None,
            functions: HashMap::new(),
            returning: false,
            breaking: false,
            continuing: false,
            stdlib_providers: vec![Box::new(crate::core::stdlib::IshCommandLine), Box::new(IshStr), Box::new(IshFS), Box::new(crate::core::stdlib::IshMath), Box::new(IshTime), Box::new(IshNet), Box::new(IshOS), Box::new(crate::core::stdlib::IshExtProc)],
            registry: Registry::new(),
            current_class: None,
            gobbler: Gobbler::new(),
            is_evaluating: false,
        }
    }

    pub fn value_to_string(&self, val: &crate::core::ast::IshValue) -> String {
        match val {
            crate::core::ast::IshValue::String(s) => s.clone(),
            crate::core::ast::IshValue::Int(i) => i.to_string(),
            crate::core::ast::IshValue::Float(f) => f.to_string(),
            crate::core::ast::IshValue::Bool(b) => b.to_string(),
            crate::core::ast::IshValue::Null => "null".to_string(),
            crate::core::ast::IshValue::Char(c) => c.to_string(),
            crate::core::ast::IshValue::TypeRef(t) => format!("<Type {}>", t),
            crate::core::ast::IshValue::Reference(id) => {
                if let Some(obj) = self.gobbler.get(*id) {
                    match obj {
                        crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                            if arr.is_empty() {
                                "object[]".to_string()
                            } else {
                                match &arr[0] {
                                    crate::core::ast::IshValue::String(_) => "string[]".to_string(),
                                    crate::core::ast::IshValue::Int(_) => "int[]".to_string(),
                                    crate::core::ast::IshValue::Float(_) => "float[]".to_string(),
                                    crate::core::ast::IshValue::Bool(_) => "bool[]".to_string(),
                                    _ => "object[]".to_string(),
                                }
                            }
                        }
                        crate::core::gobbler::HeapObject::Map(_) | crate::core::gobbler::HeapObject::Object { .. } => {
                            let class_name = if let crate::core::gobbler::HeapObject::Object { class_name, .. } = obj {
                                class_name.clone()
                            } else {
                                "Map".to_string()
                            };
                            class_name
                        }
                    }
                } else {
                    "null".to_string()
                }
            }
        }
    }

    fn pop_scope(&mut self, jobs: &mut JobController) -> Result<(), IshError> {
        self.variables.pop();
        let finalized = self.gobbler.collect(&self.variables, &self.static_variables, self.return_value.as_ref());
        for (_id, class_name, properties) in finalized {
            let class_def = self.registry.resolve_class(&class_name).cloned();
            let struct_def = if class_def.is_none() {
                self.registry.resolve_struct(&class_name).cloned()
            } else {
                None
            };
            
            let destructor_opt = if let Some(cdef) = &class_def {
                cdef.destructor.clone()
            } else if let Some(sdef) = &struct_def {
                sdef.destructor.clone()
            } else {
                None
            };
            
            let qual_name = if let Some(cdef) = &class_def {
                cdef.qualified_name.clone()
            } else if let Some(sdef) = &struct_def {
                sdef.qualified_name.clone()
            } else {
                class_name.clone()
            };

            if let Some(destructor_body) = destructor_opt {
                self.variables.push(HashMap::new());
                self.function_bases.push(self.variables.len() - 1);
                if let Some(dest_scope) = self.variables.last_mut() {
                    let temp_id = self.gobbler.allocate(HeapObject::Object {
                        class_name: class_name.clone(),
                        properties,
                    });
                    dest_scope.insert("this".to_string(), IshValue::Reference(temp_id));
                }
                let prev_class = self.current_class.clone();
                self.current_class = Some(qual_name);
                
                for stmt in destructor_body {
                    let _ = self.execute_node_with_input(&stmt, "", None, jobs);
                }
                
                self.function_bases.pop();
                if let Some(dest_scope) = self.variables.pop() {
                    if let Some(IshValue::Reference(temp_id)) = dest_scope.get("this") {
                        self.gobbler.free(*temp_id);
                    }
                }
                self.current_class = prev_class;
            }
        }
        Ok(())
    }

    fn resolve_var(&mut self, s: &str, jobs: &mut JobController) -> Result<String, IshError> {
        // Shell-style variable expansion for legacy compatibility
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
                    if "?!$0".contains(nc) {
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
                                
                                let trimmed = idx_str.trim().to_string();
                                let resolved_idx = if trimmed.starts_with('"') && trimmed.ends_with('"') {
                                    trimmed[1..trimmed.len()-1].to_string()
                                } else if trimmed.starts_with('\'') && trimmed.ends_with('\'') {
                                    trimmed[1..trimmed.len()-1].to_string()
                                } else if trimmed.starts_with('$') {
                                    self.resolve_var(&trimmed, jobs)?
                                } else if trimmed.parse::<usize>().is_ok() {
                                    trimmed
                                } else {
                                    return Err(IshError::ExecutionError(format!("Map/Array index must be in quotes, a number, or a variable. Found: [{}]", idx_str)));
                                };
                                if let IshValue::Reference(id) = &final_val {
                                    if let Some(obj) = self.gobbler.get(*id) {
                                        match obj {
                                            crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                                                if let Ok(idx) = resolved_idx.parse::<usize>() {
                                                    if let Some(v) = arr.get(idx) {
                                                        final_val = v.clone();
                                                    }
                                                }
                                            }
                                            crate::core::gobbler::HeapObject::Map(m) | crate::core::gobbler::HeapObject::Object { properties: m, .. } => {
                                                if let Some(v) = m.get(&resolved_idx) {
                                                    final_val = v.clone();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            result.push_str(&self.value_to_string(&final_val));
                            found = true;
                            break;
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
            AstNodeKind::InterpolatedString(nodes) => {
                let mut result = String::new();
                for n in nodes {
                    let eval_val = self.evaluate_node(n, jobs)?;
                    match eval_val {
                        IshValue::String(s) => result.push_str(&s),
                        IshValue::Int(i) => result.push_str(&i.to_string()),
                        IshValue::Float(f) => result.push_str(&f.to_string()),
                        IshValue::Bool(b) => result.push_str(&b.to_string()),
                        IshValue::Char(c) => result.push(c),
                        _ => result.push_str(&eval_val.to_string()),
                    }
                }
                Ok(IshValue::String(result))
            }

            AstNodeKind::CharLiteral(c) => Ok(IshValue::Char(*c)),

            AstNodeKind::Variable(var_name) => {
                if var_name == "___implicit___" {
                    return Ok(IshValue::String("___implicit___".to_string()));
                }
                
                if self.registry.classes.contains_key(var_name) || 
                self.registry.structs.contains_key(var_name) || 
                self.registry.enums.contains_key(var_name) || 
                var_name == "string" || var_name == "char" || var_name == "List" ||
                var_name == "Math" || var_name == "Time" || var_name == "Str" || 
                var_name == "FS" || var_name == "Net" || var_name == "OS" || 
                var_name == "ExtProc" || var_name == "CommandLine" 
                {   
                    return Ok(IshValue::TypeRef(var_name.clone()));
                }


                let base_idx = *self.function_bases.last().unwrap_or(&0);
                let mut search_scopes = vec![];
                for i in (base_idx..self.variables.len()).rev() {
                    search_scopes.push(i);
                }
                if base_idx != 0 {
                    search_scopes.push(0);
                }
                for i in &search_scopes {
                    if let Some(val) = self.variables[*i].get(var_name) {
                        return Ok(val.clone());
                    }
                }

                // Not found locally. Are we in a class Context? (Implicit member access)
                if let Some(class_qn) = &self.current_class {
                    if let Some(class_def) = self.registry.resolve_class(class_qn) {
                        if let Some(field_def) = class_def.fields.get(var_name) {
                            if field_def.is_static {
                                let static_key = format!("{}::{}", class_def.qualified_name, var_name);
                                if let Some(val) = self.static_variables.get(&static_key) {
                                    return Ok(val.clone());
                                }
                                return Ok(IshValue::Null);
                            } else {
                                // Instance field: find "this" in scopes
                                let mut this_id = None;
                                for i in &search_scopes {
                                    if let Some(IshValue::Reference(id)) = self.variables[*i].get("this") {
                                        this_id = Some(*id);
                                        break;
                                    }
                                }
                                if let Some(id) = this_id {
                                    if let Some(crate::core::gobbler::HeapObject::Object { properties, .. }) = self.gobbler.get(id) {
                                        if let Some(val) = properties.get(var_name) {
                                            return Ok(val.clone());
                                        }
                                        return Ok(IshValue::Null);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(struct_def) = self.registry.resolve_struct(class_qn) {
                        if let Some(field_def) = struct_def.fields.get(var_name) {
                            if field_def.is_static {
                                let static_key = format!("{}::{}", struct_def.qualified_name, var_name);
                                if let Some(val) = self.static_variables.get(&static_key) {
                                    return Ok(val.clone());
                                }
                                return Ok(IshValue::Null);
                            } else {
                                let mut this_id = None;
                                for i in &search_scopes {
                                    if let Some(IshValue::Reference(id)) = self.variables[*i].get("this") {
                                        this_id = Some(*id);
                                        break;
                                    }
                                }
                                if let Some(id) = this_id {
                                    if let Some(crate::core::gobbler::HeapObject::Object { properties, .. }) = self.gobbler.get(id) {
                                        if let Some(val) = properties.get(var_name) {
                                            return Ok(val.clone());
                                        }
                                        return Ok(IshValue::Null);
                                    }
                                }
                            }
                        }
                    }
                }

                // Check static access explicitly via ClassName::Field
                if var_name.contains("::") {
                    if let Some(val) = self.static_variables.get(var_name) {
                        return Ok(val.clone());
                    }
                }

                // Allow class names and stdlib providers to be used as string values
                if self.registry.classes.contains_key(var_name) || self.registry.structs.contains_key(var_name) || self.registry.enums.contains_key(var_name) || var_name == "CommandLine" || var_name == "Math" || var_name == "Str" || var_name == "Time" || var_name == "Net" || var_name == "OS" || var_name == "ExtProc" || var_name == "FS" {
                    return Ok(IshValue::String(var_name.clone()));
                }

                Err(crate::error::IshError::ExecutionError(format!("Variable '{}' is not defined in this scope", var_name)))
            }
            AstNodeKind::IndexAccess { object, index } => {
                let obj_val = self.evaluate_node(object, jobs)?;
                let index_val = self.evaluate_node(index, jobs)?;

                if let IshValue::String(s) = &obj_val {
                    if let IshValue::Int(i) = index_val {
                        if i >= 0 && (i as usize) < s.chars().count() {
                            return Ok(IshValue::Char(s.chars().nth(i as usize).unwrap()));
                        }
                        return Err(IshError::ExecutionError(format!("IndexOutOfRangeException: {}", i)));
                    }
                    return Err(IshError::ExecutionError("String index must be an integer".into()));
                }

                if let IshValue::Reference(id) = obj_val {
                    if let Some(heap_obj) = self.gobbler.get(id) {
                        match heap_obj {
                            crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                                let idx_str = index_val.to_string();
                                if let Ok(idx) = idx_str.parse::<usize>() {
                                    if idx < arr.len() {
                                        return Ok(arr[idx].clone());
                                    } else {
                                        return Err(IshError::ExecutionError(format!("IndexOutOfRangeException: {}", idx)));
                                    }
                                } else {
                                    return Err(IshError::ExecutionError(format!("TypeError: Invalid array index {}", idx_str)));
                                }
                            }
                            _ => return Err(IshError::ExecutionError(format!("TypeError: Cannot index into non-array/list type"))),
                        }
                    }
                }
                Err(IshError::ExecutionError(format!("TypeError: Cannot index into non-array/list type")))
            }
            AstNodeKind::PropertyAccess { object, property_name } => {
                let obj_val = self.evaluate_node(object, jobs)?;

                if let IshValue::TypeRef(s) = &obj_val {
                    if s == "string" && property_name == "Empty" {
                        return Ok(IshValue::String(String::new()));
                    }
                }
                
                if let IshValue::String(s) = &obj_val {
                    // Check Enum
                    if let Some(enum_def) = self.registry.enums.get(s) {
                        if let Some(index) = enum_def.variants.iter().position(|v| v == property_name) {
                            return Ok(IshValue::Int(index as i32));
                        } else {
                            return Err(IshError::ExecutionError(format!("Enum {} does not have variant {}", s, property_name)));
                        }
                    }

                    // Check Static Class Field Access
                    if let Some(class_def) = self.registry.resolve_class(s) {
                        if let Some(field_def) = class_def.fields.get(property_name.as_str()) {
                            if field_def.is_static {
                                Registry::check_access(&field_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;
                                let static_key = format!("{}::{}", class_def.qualified_name, property_name);
                                return Ok(self.static_variables.get(&static_key).cloned().unwrap_or(IshValue::Null));
                            } else {
                                return Err(IshError::ExecutionError(format!("Cannot access instance field '{}' of '{}' statically", property_name, s)));
                            }
                        }
                    }

                    if let Some(struct_def) = self.registry.resolve_struct(s) {
                        if let Some(field_def) = struct_def.fields.get(property_name.as_str()) {
                            if field_def.is_static {
                                Registry::check_access(&field_def.access, self.current_class.as_deref(), &struct_def.qualified_name)?;
                                let static_key = format!("{}::{}", struct_def.qualified_name, property_name);
                                return Ok(self.static_variables.get(&static_key).cloned().unwrap_or(IshValue::Null));
                            } else {
                                return Err(IshError::ExecutionError(format!("Cannot access instance field '{}' of '{}' statically", property_name, s)));
                            }
                        }
                    }
                }

                if let IshValue::Reference(id) = &obj_val {
                    if let Some(heap_obj) = self.gobbler.get(*id) {
                        match heap_obj {
                            crate::core::gobbler::HeapObject::Object { class_name, properties } => {
                                if let Some(class_def) = self.registry.resolve_class(class_name) {
                                    if let Some(field_def) = class_def.fields.get(property_name.as_str()) {
                                        Registry::check_access(&field_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;
                                    }
                                } else if let Some(struct_def) = self.registry.resolve_struct(class_name) {
                                    if let Some(field_def) = struct_def.fields.get(property_name.as_str()) {
                                        Registry::check_access(&field_def.access, self.current_class.as_deref(), &struct_def.qualified_name)?;
                                    }
                                }
                                return Ok(properties.get(property_name.as_str()).cloned().unwrap_or(IshValue::Null));
                            }
                            crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                                if property_name == "Length" {
                                    return Ok(IshValue::Int(arr.len() as i32));
                                }
                            }
                            crate::core::gobbler::HeapObject::Map(m) => {
                                if property_name == "Count" {
                                    return Ok(IshValue::Int(m.len() as i32));
                                }
                                return Ok(m.get(property_name.as_str()).cloned().unwrap_or(IshValue::Null));
                            }
                            _ => {}
                        }
                    }
                }
                Err(IshError::ExecutionError(format!("Cannot access property '{}' on non-object value '{}'.", property_name, obj_val.to_string())))
            }
            AstNodeKind::StringLiteral(s) => {
                if (s.starts_with('$') || s == "this") && !s.contains(" ") && !s.contains("(") && !s.contains("{") {
                    let var_name = if s == "this" { "this" } else { &s[1..] };
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

            AstNodeKind::BinaryOp { left, operator, right } => {
                let left_val = self.evaluate_node(left, jobs)?;
                let right_val = self.evaluate_node(right, jobs)?;
                
                match operator.as_str() {
                    "+" => {
                        return match (left_val, right_val) {
                            (IshValue::Int(l), IshValue::Int(r)) => Ok(IshValue::Int(l + r)),
                            (IshValue::Float(l), IshValue::Float(r)) => Ok(IshValue::Float(l + r)),
                            (IshValue::Int(l), IshValue::Float(r)) => Ok(IshValue::Float(l as f32 + r)),
                            (IshValue::Float(l), IshValue::Int(r)) => Ok(IshValue::Float(l + r as f32)),
                            (l, r) => Ok(IshValue::String(format!("{}{}", l.to_string(), r.to_string()))), // String concat
                        };
                    }
                    "-" => {
                        return match (left_val, right_val) {
                            (IshValue::Int(l), IshValue::Int(r)) => Ok(IshValue::Int(l - r)),
                            (IshValue::Float(l), IshValue::Float(r)) => Ok(IshValue::Float(l - r)),
                            (IshValue::Int(l), IshValue::Float(r)) => Ok(IshValue::Float(l as f32 - r)),
                            (IshValue::Float(l), IshValue::Int(r)) => Ok(IshValue::Float(l - r as f32)),
                            _ => Err(IshError::ExecutionError(format!("Cannot subtract these types"))),
                        };
                    }
                    "*" => {
                        return match (left_val, right_val) {
                            (IshValue::Int(l), IshValue::Int(r)) => Ok(IshValue::Int(l * r)),
                            (IshValue::Float(l), IshValue::Float(r)) => Ok(IshValue::Float(l * r)),
                            (IshValue::Int(l), IshValue::Float(r)) => Ok(IshValue::Float(l as f32 * r)),
                            (IshValue::Float(l), IshValue::Int(r)) => Ok(IshValue::Float(l * r as f32)),
                            _ => Err(IshError::ExecutionError(format!("Cannot multiply these types"))),
                        };
                    }
                    "/" => {
                        return match (left_val, right_val) {
                            (IshValue::Int(l), IshValue::Int(r)) => {
                                if r == 0 { Err(IshError::ExecutionError("Division by zero".into())) } else { Ok(IshValue::Int(l / r)) }
                            }
                            (IshValue::Float(l), IshValue::Float(r)) => Ok(IshValue::Float(l / r)),
                            (IshValue::Int(l), IshValue::Float(r)) => Ok(IshValue::Float(l as f32 / r)),
                            (IshValue::Float(l), IshValue::Int(r)) => Ok(IshValue::Float(l / r as f32)),
                            _ => Err(IshError::ExecutionError(format!("Cannot divide these types"))),
                        };
                    }
                    "%" => {
                        return match (left_val, right_val) {
                            (IshValue::Int(l), IshValue::Int(r)) => {
                                if r == 0 { Err(IshError::ExecutionError("Modulo by zero".into())) } else { Ok(IshValue::Int(l % r)) }
                            }
                            (IshValue::Float(l), IshValue::Float(r)) => Ok(IshValue::Float(l % r)),
                            (IshValue::Int(l), IshValue::Float(r)) => Ok(IshValue::Float(l as f32 % r)),
                            (IshValue::Float(l), IshValue::Int(r)) => Ok(IshValue::Float(l % r as f32)),
                            _ => Err(IshError::ExecutionError(format!("Cannot modulo these types"))),
                        };
                    }
                    "&&" => {
                        let l_bool = match left_val {
                            IshValue::Bool(b) => b,
                            _ => true,
                        };
                        let r_bool = match right_val {
                            IshValue::Bool(b) => b,
                            _ => true,
                        };
                        return Ok(IshValue::Bool(l_bool && r_bool));
                    }
                    "||" => {
                        let l_bool = match left_val {
                            IshValue::Bool(b) => b,
                            _ => true,
                        };
                        let r_bool = match right_val {
                            IshValue::Bool(b) => b,
                            _ => true,
                        };
                        return Ok(IshValue::Bool(l_bool || r_bool));
                    }
                    _ => {}
                }

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
            AstNodeKind::UnaryOp { operator, operand } => {
                let val = self.evaluate_node(operand, jobs)?;
                match operator.as_str() {
                    "!" => {
                        match val {
                            IshValue::Bool(b) => Ok(IshValue::Bool(!b)),
                            _ => Ok(IshValue::Bool(false)),
                        }
                    }
                    "-" => {
                        match val {
                            IshValue::Int(i) => Ok(IshValue::Int(-i)),
                            IshValue::Float(f) => Ok(IshValue::Float(-f)),
                            _ => Err(IshError::ExecutionError("Cannot negate non-numeric type".into())),
                        }
                    }
                    _ => Err(IshError::ExecutionError(format!("Unknown unary operator: {}", operator))),
                }
            }
            AstNodeKind::TernaryOp { condition, true_value, false_value } => {
                let cond_val = self.evaluate_node(condition, jobs)?;
                let is_true = match cond_val {
                    IshValue::Bool(b) => b,
                    IshValue::Null => false,
                    _ => true,
                };
                if is_true {
                    self.evaluate_node(true_value, jobs)
                } else {
                    self.evaluate_node(false_value, jobs)
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

        // Initialize static variables before actual execution
        let classes = self.registry.classes.clone();
        for (class_qn, class_def) in &classes {
            for (field_name, field_def) in &class_def.fields {
                if field_def.is_static {
                    let val = if let Some(default_node) = &field_def.default_value {
                        self.evaluate_node(default_node, jobs)?
                    } else {
                        IshValue::Null
                    };
                    self.static_variables.insert(format!("{}::{}", class_qn, field_name), val);
                }
            }
        }
        
        let structs = self.registry.structs.clone();
        for (struct_qn, struct_def) in &structs {
            for (field_name, field_def) in &struct_def.fields {
                if field_def.is_static {
                    let val = if let Some(default_node) = &field_def.default_value {
                        self.evaluate_node(default_node, jobs)?
                    } else {
                        IshValue::Null
                    };
                    self.static_variables.insert(format!("{}::{}", struct_qn, field_name), val);
                }
            }
        }

        // Enforce OOP requirement
        match self.registry.find_entry_point() {
            Ok(program_class_name) => {
                // First, execute all declarations so methods get registered in self.functions
                self.execute_node_with_input(ast, "", None, jobs)?;
                
                // Now invoke Program::Main
                let class_def = self.registry.classes.get(&program_class_name).cloned()
                    .ok_or_else(|| IshError::ExecutionError(
                        "Internal error: Program class disappeared after registration.".to_string()
                    ))?;
                    
                let main_method = class_def.methods.get("Main")
                    .ok_or_else(|| IshError::ExecutionError(
                        "Internal error: Main method disappeared after registration.".to_string()
                    ))?;
                    
                if !main_method.is_static {
                    return Err(IshError::ExecutionError("Main method must be static.".to_string()));
                }
                
                // Execute Main method body
                self.current_class = Some("Program".to_string());
                self.variables.push(HashMap::new());
                self.function_bases.push(self.variables.len() - 1);
                
                // Add args
                let mut arg_list = Vec::new();
                for arg in &self.script_args {
                    arg_list.push(IshValue::String(arg.clone()));
                }
                let args_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(arg_list));
                if let Some(scope) = self.variables.last_mut() {
                    scope.insert("args".to_string(), IshValue::Reference(args_ref));
                }

                let _last_output = String::new();
                for stmt in &main_method.body {
                    self.execute_node_with_input(stmt, "", None, jobs)?;
                    if self.returning { break; }
                }
                
                let ret_val = self.return_value.take();
                
                self.returning = false;
                self.function_bases.pop();
                let _ = self.pop_scope(jobs);
                self.current_class = None;
                
                if let Some(IshValue::Int(code)) = ret_val {
                    self.last_exit_code = code;
                }
                
                return Ok(true);
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    pub fn resolve_callable(&self, program: &str) -> Option<(Vec<crate::core::ast::Param>, Vec<AstNode>)> {
        if let Some(AstNode { kind: AstNodeKind::Function { params, body, .. }, .. }) = self.functions.get(program).cloned() {
            return Some((params, body));
        }
        if let Some(class_name) = &self.current_class {
            if let Some(class_def) = self.registry.resolve_class(class_name) {
                if let Some(method) = class_def.methods.get(program) {
                    return Some((method.params.clone(), method.body.clone()));
                }
            }
        }
        None
    }

    pub fn execute_node_with_input(&mut self, node: &AstNode, input: &str, out_file: Option<&str>, jobs: &mut JobController) -> Result<bool, IshError>
    {
        match &node.kind {
            AstNodeKind::CharLiteral(_) => {
                let val = self.evaluate_node(node, jobs)?;
                let output = val.to_string();
                if let Some(path) = out_file {
                    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                    let _ = file.write_all(output.as_bytes());
                } else if !output.is_empty() {
                    print!("{}", output);
                    let _ = std::io::stdout().flush();
                }
                return Ok(true)
            }
            AstNodeKind::InterpolatedString(_) | AstNodeKind::BinaryOp { .. } | AstNodeKind::UnaryOp { .. } | AstNodeKind::TernaryOp { .. } => {
                let val = self.evaluate_node(node, jobs)?;
                let success = match val {
                    IshValue::Bool(b) => b,
                    _ => true,
                };
                self.last_exit_code = if success { 0 } else { 1 };
                return Ok(success)
            }

            AstNodeKind::Assignment { type_specifier, variable, index, value, is_declaration } => {
                let val = match &value.kind {
                    AstNodeKind::Array(items) => {
                        let mut arr = Vec::new();
                        for item in items {
                            arr.push(self.evaluate_node(item, jobs)?);
                        }
                        let arr_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(arr));
                        IshValue::Reference(arr_ref)
                    }
                    AstNodeKind::Map(items) => {
                        let mut m = HashMap::new();
                        for (k, v) in items {
                            m.insert(k.clone(), self.evaluate_node(v, jobs)?);
                        }
                        let map_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Map(m));
                        IshValue::Reference(map_ref)
                    }
                    _ => self.evaluate_node(value, jobs)?,
                };
                
                if let Some(idx_node) = index {
                    let idx_val = self.evaluate_node(idx_node, jobs)?;
                    let mut found = false;
                    for scope in self.variables.iter().rev() {
                        if let Some(existing) = scope.get(variable) {
                            if let IshValue::Reference(id) = existing {
                                found = true;
                                if let Some(heap_obj) = self.gobbler.get_mut(*id) {
                                    match heap_obj {
                                        crate::core::gobbler::HeapObject::Array(_) => return Err(IshError::ExecutionError("Array is immutable. Use List for mutable ordered collections.".to_string())),
                                        crate::core::gobbler::HeapObject::List(l) => {
                                            if let IshValue::Int(i) = idx_val {
                                                if i >= 0 && (i as usize) < l.len() {
                                                    l[i as usize] = val.clone();
                                                } else {
                                                    return Err(IshError::ExecutionError("Index out of bounds".to_string()));
                                                }
                                            } else {
                                                return Err(IshError::ExecutionError("List index must be an integer".to_string()));
                                            }
                                        }
                                        crate::core::gobbler::HeapObject::Map(m) => {
                                            m.insert(idx_val.to_string(), val.clone());
                                        }
                                        _ => return Err(IshError::ExecutionError("Variable is not indexable".to_string())),
                                    }
                                }
                            } else {
                                return Err(IshError::ExecutionError("Variable is not indexable".to_string()));
                            }
                            break;
                        }
                    }
                    if !found { return Err(IshError::ExecutionError(format!("Variable {} not found", variable))); }
                    return Ok(true);
                }

                if *is_declaration {
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), val);
                    }
                } else {
                    if variable.contains('.') {
                        let parts: Vec<&str> = variable.split('.').collect();
                        let obj_name = parts[0];
                        let prop_name = parts[1];
                        
                        let mut obj_id = None;
                        for scope in self.variables.iter().rev() {
                            if let Some(existing) = scope.get(obj_name) {
                                if let IshValue::Reference(id) = existing {
                                    obj_id = Some(*id);
                                }
                                break;
                            }
                        }
                        
                        if let Some(id) = obj_id {
                            if let Some(heap_obj) = self.gobbler.get_mut(id) {
                                match heap_obj {
                                    crate::core::gobbler::HeapObject::Map(m) => {
                                        m.insert(prop_name.to_string(), val.clone());
                                        return Ok(true);
                                    }
                                    crate::core::gobbler::HeapObject::Object { class_name, properties } => {
                                        if let Some(class_def) = self.registry.resolve_class(class_name) {
                                            if let Some(field_def) = class_def.fields.get(prop_name) {
                                                Registry::check_access(&field_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;
                                            }
                                        } else if let Some(struct_def) = self.registry.resolve_struct(class_name) {
                                            if let Some(field_def) = struct_def.fields.get(prop_name) {
                                                Registry::check_access(&field_def.access, self.current_class.as_deref(), &struct_def.qualified_name)?;
                                            }
                                        }
                                        properties.insert(prop_name.to_string(), val.clone());
                                        return Ok(true);
                                    }
                                    _ => return Err(IshError::ExecutionError(format!("Cannot assign property on non-object '{}'", obj_name))),
                                }
                            }
                        } else {
                            if let Some(class_def) = self.registry.resolve_class(obj_name) {
                                if let Some(field_def) = class_def.fields.get(prop_name) {
                                    if field_def.is_static {
                                        Registry::check_access(&field_def.access, self.current_class.as_deref(), &class_def.qualified_name)?;
                                        let static_key = format!("{}::{}", class_def.qualified_name, prop_name);
                                        self.static_variables.insert(static_key, val.clone());
                                        return Ok(true);
                                    } else {
                                        return Err(IshError::ExecutionError(format!("Cannot access instance field '{}' of '{}' statically", prop_name, obj_name)));
                                    }
                                }
                            }
                            
                            return Err(IshError::ExecutionError(format!("Object or class '{}' not found", obj_name)));
                        }
                    }

                    let mut found = false;
                    for scope in self.variables.iter_mut().rev() {
                        if scope.contains_key(variable) {
                            scope.insert(variable.clone(), val.clone());
                            found = true;
                            break;
                        }
                    }
                    
                    if !found {
                        if let Some(class_qn) = &self.current_class {
                            let mut is_member = false;
                            let mut is_static = false;
                            
                            if let Some(class_def) = self.registry.resolve_class(class_qn) {
                                if let Some(field_def) = class_def.fields.get(variable) {
                                    is_member = true;
                                    is_static = field_def.is_static;
                                }
                            } else if let Some(struct_def) = self.registry.resolve_struct(class_qn) {
                                if let Some(field_def) = struct_def.fields.get(variable) {
                                    is_member = true;
                                    is_static = field_def.is_static;
                                }
                            }
                            
                            if is_member {
                                if is_static {
                                    let static_key = format!("{}::{}", class_qn, variable);
                                    self.static_variables.insert(static_key, val.clone());
                                    found = true;
                                } else {
                                    let mut this_id = None;
                                    for scope in self.variables.iter().rev() {
                                        if let Some(IshValue::Reference(id)) = scope.get("this") {
                                            this_id = Some(*id);
                                            break;
                                        }
                                    }
                                    if let Some(id) = this_id {
                                        if let Some(crate::core::gobbler::HeapObject::Object { properties, .. }) = self.gobbler.get_mut(id) {
                                            properties.insert(variable.clone(), val.clone());
                                            found = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    if !found {
                        if variable.contains("::") {
                            if self.static_variables.contains_key(variable) {
                                self.static_variables.insert(variable.clone(), val.clone());
                                found = true;
                            }
                        }
                    }

                    if !found {
                        return Err(IshError::ExecutionError(format!("Variable '{}' is not declared. Use 'declare {} = ...' to declare it.", variable, variable)));
                    }
                }
                return Ok(true)
            }
            AstNodeKind::Switch { expression, cases, default_case } => {
                let expr_val = self.evaluate_node(expression, jobs)?;
                let mut matched = false;
                for (case_expr, body) in cases {
                    let case_val = self.evaluate_node(case_expr, jobs)?;
                    if expr_val == case_val {
                        matched = true;
                        self.variables.push(std::collections::HashMap::new());
                        for stmt in body {
                            self.execute_node_with_input(stmt, "", out_file, jobs)?;
                            if self.returning || self.breaking || self.continuing { break; }
                        }
                        self.variables.pop();
                        break;
                    }
                }
                if !matched {
                    if let Some(body) = default_case {
                        self.variables.push(std::collections::HashMap::new());
                        for stmt in body {
                            self.execute_node_with_input(stmt, "", out_file, jobs)?;
                            if self.returning || self.breaking || self.continuing { break; }
                        }
                        self.variables.pop();
                    }
                }
                if self.breaking { self.breaking = false; }
                return Ok(true)
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
                    let _ = self.pop_scope(jobs);
                } else if let Some(else_stmts) = else_body {
                    self.variables.push(HashMap::new());
                    for stmt in else_stmts {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    let _ = self.pop_scope(jobs);
                }
                return Ok(block_success)
            }
            AstNodeKind::For { variable, iterable, body } => {
                let iterable_val = self.evaluate_node(iterable, jobs)?;
                let items: Vec<IshValue> = match iterable_val {
                    IshValue::Reference(id) => {
                        if let Some(crate::core::gobbler::HeapObject::Array(arr)) = self.gobbler.get(id) {
                            arr.clone()
                        } else if let Some(crate::core::gobbler::HeapObject::List(arr)) = self.gobbler.get(id) {
                            arr.clone()
                        } else if let Some(crate::core::gobbler::HeapObject::Map(map)) = self.gobbler.get(id) {
                            let mut pairs = Vec::new();
                            for (k, v) in map.clone() {
                                let mut properties = HashMap::new();
                                properties.insert("Key".to_string(), IshValue::String(k));
                                properties.insert("Value".to_string(), v);
                                let pair_id = self.gobbler.allocate(crate::core::gobbler::HeapObject::Object {
                                    class_name: "Pair".to_string(),
                                    properties
                                });
                                pairs.push(IshValue::Reference(pair_id));
                            }
                            pairs
                        } else {
                            vec![IshValue::Reference(id)]
                        }
                    }
                    IshValue::String(s) => s.chars().map(|c| IshValue::Char(c)).collect(),
                    _ => vec![iterable_val.clone()],
                };
                
                // Protect items from being garbage collected during loop iterations
                let temp_array_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(items.clone()));
                let temp_var_name = format!("__temp_for_{}", temp_array_ref);
                if let Some(scope) = self.variables.last_mut() {
                    scope.insert(temp_var_name.clone(), IshValue::Reference(temp_array_ref));
                }

                let mut block_success = true;
                for item in items {
                    if self.returning || self.breaking { break; }
                    self.variables.push(HashMap::new());
                    if let Some(scope) = self.variables.last_mut() {
                        scope.insert(variable.clone(), item);
                    }
                    for stmt in body {
                        block_success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                        if self.returning || self.breaking || self.continuing { break; }
                    }
                    let _ = self.pop_scope(jobs);
                    if self.continuing {
                        self.continuing = false;
                    }
                    if self.breaking || self.returning { break; }
                }
                
                if let Some(scope) = self.variables.last_mut() {
                    scope.remove(&temp_var_name);
                }

                if self.breaking { self.breaking = false; }
                return Ok(block_success)
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
                    let _ = self.pop_scope(jobs);
                    if self.continuing {
                        self.continuing = false;
                    }
                    if self.breaking || self.returning { break; }
                }
                if self.breaking { self.breaking = false; }
                return Ok(block_success)
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
                            let _ = self.pop_scope(jobs); // pop try block scope
                            
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
                            let _ = self.pop_scope(jobs); // pop catch block scope
                            self.last_exit_code = if catch_success { 0 } else { 1 };
                            return Ok(catch_success);
                        }
                    }
                    if self.returning || self.breaking || self.continuing { break; }
                }
                let _ = self.pop_scope(jobs);
                self.last_exit_code = if success { 0 } else { 1 };
                return Ok(success)
            }
            AstNodeKind::Break => {
                self.breaking = true;
                return Ok(true)
            }
            AstNodeKind::Continue => {
                self.continuing = true;
                return Ok(true)
            }
            AstNodeKind::FieldDecl { name, default_value, .. } => {
                let val = if let Some(def) = default_value {
                    self.evaluate_node(def, jobs)?
                } else {
                    IshValue::Null
                };
                if let Some(scope) = self.variables.last_mut() {
                    scope.insert(name.clone(), val);
                }
                return Ok(true)
            }
            AstNodeKind::Function { name, .. } => {
                self.functions.insert(name.clone(), node.clone());
                return Ok(true)
            }
            AstNodeKind::Array(_) | AstNodeKind::Map(_) => {
                return Ok(true)
            }
            AstNodeKind::Variable(var_name) => {
                let val = self.evaluate_node(node, jobs)?;
                let output = val.to_string();
                if let Some(path) = out_file {
                    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                    let _ = file.write_all(output.as_bytes());
                } else if !output.is_empty() {
                    print!("{}", output);
                    let _ = std::io::stdout().flush();
                }
                return Ok(true)
            }
            AstNodeKind::PropertyAccess { .. } | AstNodeKind::IndexAccess { .. } => {
                let val = self.evaluate_node(node, jobs)?;
                let output = val.to_string();
                if let Some(path) = out_file {
                    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
                    let _ = file.write_all(output.as_bytes());
                } else if !output.is_empty() {
                    print!("{}", output);
                    let _ = std::io::stdout().flush();
                }
                self.return_value = Some(val);
                return Ok(true)
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
                return Ok(true)
            }
            AstNodeKind::Return(inner) => {
                let val = self.evaluate_node(inner, jobs)?;
                self.return_value = Some(val);
                self.returning = true;
                return Ok(true)
            }

            AstNodeKind::NamespaceDecl { body, .. } => {
                for stmt in body {
                    self.execute_node_with_input(stmt, input, out_file, jobs)?;
                    if self.returning || self.breaking || self.continuing { break; }
                }
                return Ok(true)
            }
            AstNodeKind::ClassDecl { name, methods, .. } => {
                for method_node in methods {
                    if let AstNodeKind::Function { name: mname, .. } = &method_node.kind {
                        let qualified = format!("{}::{}", name, mname);
                        self.functions.insert(qualified, method_node.clone());
                    }
                }
                return Ok(true)
            }
            AstNodeKind::StructDecl { .. } => {
                return Ok(true)
            }

            AstNodeKind::ObjectInstantiation { class_name, args, initializer } => {
                let base_class_name = if let Some(idx) = class_name.find('<') {
                    &class_name[..idx]
                } else {
                    class_name.as_str()
                };

                if base_class_name == "List" {
                    let mut list_elements = Vec::new();
                    if let Some(init) = initializer {
                        for node in init {
                            if let AstNodeKind::KeyValuePair { .. } = &node.kind {
                                return Err(IshError::ExecutionError("List initializer cannot contain key-value pairs".to_string()));
                            }
                            list_elements.push(self.evaluate_node(node, jobs)?);
                        }
                    }
                    let list_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::List(list_elements));
                    self.return_value = Some(IshValue::Reference(list_ref));
                    return Ok(true);
                }

                if base_class_name == "Map" {
                    let mut map_elements = HashMap::new();
                    if let Some(init) = initializer {
                        for node in init {
                            if let AstNodeKind::KeyValuePair { key, value } = &node.kind {
                                let key_val = self.evaluate_node(key, jobs)?;
                                let val_val = self.evaluate_node(value, jobs)?;
                                let key_str = match key_val {
                                    IshValue::String(s) => s,
                                    _ => key_val.to_string(),
                                };
                                map_elements.insert(key_str, val_val);
                            } else {
                                return Err(IshError::ExecutionError("Map initializer must contain key-value pairs".to_string()));
                            }
                        }
                    }
                    let map_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Map(map_elements));
                    self.return_value = Some(IshValue::Reference(map_ref));
                    return Ok(true);
                }

                if initializer.is_some() {
                    return Err(IshError::ExecutionError(format!("Initializer blocks are not supported for class/struct '{}'", class_name)));
                }
                
                let class_def = self.registry.resolve_class(base_class_name).cloned();
                let struct_def = if class_def.is_none() {
                    self.registry.resolve_struct(base_class_name).cloned()
                } else {
                    None
                };

                if class_def.is_none() && struct_def.is_none() {
                    return Err(IshError::ExecutionError(format!(
                        "Class or Struct '{}' is not defined.", class_name
                    )));
                }

                if let Some(cdef) = &class_def {
                    if cdef.is_static {
                        return Err(IshError::ExecutionError(format!(
                            "Cannot instantiate static class '{}'.", class_name
                        )));
                    }
                    Registry::check_access(&cdef.access, self.current_class.as_deref(), &cdef.qualified_name)?;
                } else if let Some(sdef) = &struct_def {
                    Registry::check_access(&sdef.access, self.current_class.as_deref(), &sdef.qualified_name)?;
                }

                let fields = if let Some(cdef) = &class_def { &cdef.fields } else { &struct_def.as_ref().unwrap().fields };
                let constructor = if let Some(cdef) = &class_def { &cdef.constructor } else { &struct_def.as_ref().unwrap().constructor };

                let mut properties = HashMap::new();
                for (fname, fdef) in fields {
                    if !fdef.is_static { // Only initialize instance fields
                        let val = if let Some(default_node) = &fdef.default_value {
                            self.evaluate_node(default_node, jobs)?
                        } else {
                            IshValue::Null
                        };
                        properties.insert(fname.clone(), val);
                    }
                }

                let mut eval_args = Vec::new();
                for arg_node in args {
                    eval_args.push(self.evaluate_node(arg_node, jobs)?);
                }

                let mut evaluated_params = Vec::new();
                if let Some((_, params, _)) = constructor {
                    let mut arg_idx = 0;
                    for param in params {
                        if param.is_variadic {
                            let mut variadic_arr = Vec::new();
                            while arg_idx < eval_args.len() {
                                variadic_arr.push(eval_args[arg_idx].clone());
                                arg_idx += 1;
                            }
                            let arr_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(variadic_arr));
                            evaluated_params.push((param.name.clone(), IshValue::Reference(arr_ref)));
                        } else {
                            if arg_idx < eval_args.len() {
                                evaluated_params.push((param.name.clone(), eval_args[arg_idx].clone()));
                                arg_idx += 1;
                            } else {
                                if let Some(default_expr) = &param.default_value {
                                    let val = self.evaluate_node(default_expr, jobs)?;
                                    evaluated_params.push((param.name.clone(), val));
                                } else {
                                    evaluated_params.push((param.name.clone(), IshValue::Null));
                                }
                            }
                        }
                    }
                }

                if let Some((acc, _, constructor_body)) = constructor {
                    let qual_name = if let Some(cdef) = &class_def { &cdef.qualified_name } else { &struct_def.as_ref().unwrap().qualified_name };
                    Registry::check_access(acc, self.current_class.as_deref(), qual_name)?;

                    self.variables.push(HashMap::new());
                    self.function_bases.push(self.variables.len() - 1);
                    if let Some(scope) = self.variables.last_mut() {
                        let obj_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Object {
                            class_name: class_name.clone(),
                            properties: properties.clone(),
                        });
                        scope.insert("this".to_string(), IshValue::Reference(obj_ref));
                        
                        for (pname, pval) in evaluated_params {
                            scope.insert(pname, pval);
                        }
                    }
                    let prev_class = self.current_class.clone();
                    self.current_class = Some(qual_name.clone());
                    for stmt in constructor_body {
                        self.execute_node_with_input(stmt, "", None, jobs)?;
                        if self.returning { break; }
                    }
                    let mut final_obj_ref = 0;
                    if let Some(scope) = self.variables.last() {
                        if let Some(IshValue::Reference(id)) = scope.get("this") {
                            final_obj_ref = *id;
                        }
                    }
                    self.return_value = Some(IshValue::Reference(final_obj_ref));

                    self.returning = false;
                    self.function_bases.pop();
                    let _ = self.pop_scope(jobs);
                    self.current_class = prev_class;
                    return Ok(true);
                }

                let obj_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Object {
                    class_name: class_name.clone(),
                    properties,
                });
                self.return_value = Some(IshValue::Reference(obj_ref));
                return Ok(true)
            }

            AstNodeKind::MethodCall { object, method_name, args } => {
                let mut eval_args = Vec::new();

                for arg_node in args {
                    eval_args.push(self.evaluate_node(arg_node, jobs)?);
                }

                let mut mutated = false;
            
                if let AstNodeKind::Variable(var_name) = &object.kind {
                    if method_name == "Append" || method_name == "AppendTo" || method_name == "Clear" {
                        for scope in self.variables.iter_mut().rev() {
                            if let Some(existing) = scope.get_mut(var_name) {
                                if let IshValue::String(s) = existing {
                                    mutated = true;
                                    if method_name == "Clear" {
                                        s.clear();
                                    } else if let Some(arg) = eval_args.get(0) {
                                        s.push_str(&arg.to_string());
                                    }
                                }
                                break;
                            }
                        }
                        if mutated {
                            self.last_exit_code = 0;
                            return Ok(true);
                        }
                    }
                }

                let mut obj_val = self.evaluate_node(object, jobs)?;
                
                if let IshValue::String(ref s) = obj_val {
                    if s == "___implicit___" {
                        let mut has_this = false;
                        for scope in self.variables.iter().rev() {
                            if let Some(t) = scope.get("this") {
                                obj_val = t.clone();
                                has_this = true;
                                break;
                            }
                        }
                        if !has_this {
                            if let Some(c) = &self.current_class {
                                obj_val = IshValue::String(c.clone());
                            } else {
                                return Err(IshError::ExecutionError(format!("Implicit method call '{}' outside of class.", method_name)));
                            }
                        }
                    }
                }
                
                let mut eval_args = Vec::new();
                for arg_node in args {
                    eval_args.push(self.evaluate_node(arg_node, jobs)?);
                }

                if let IshValue::Reference(id) = obj_val {
                    let mut obj_class_name = None;
                    
                    if let Some(heap_obj) = self.gobbler.get_mut(id) {
                        match heap_obj {
                            crate::core::gobbler::HeapObject::List(l) => {
                                match method_name.as_str() {
                                    "add" => {
                                        if let Some(arg) = eval_args.get(0) {
                                            l.push(arg.clone());
                                        }
                                    }
                                    "remove" => {
                                        if let Some(idx) = eval_args.get(0) {
                                            if let IshValue::Int(i) = idx {
                                                if *i >= 0 && (*i as usize) < l.len() {
                                                    l.remove(*i as usize);
                                                }
                                            }
                                        }
                                    }
                                    "clear" => l.clear(),
                                    _ => return Err(IshError::ExecutionError(format!("List has no method '{}'", method_name))),
                                }
                                return Ok(true);
                            }
                            crate::core::gobbler::HeapObject::Object { class_name, .. } => {
                                obj_class_name = Some(class_name.clone());
                            }
                            _ => return Err(IshError::ExecutionError(format!("Method call on unsupported heap object"))),
                        }
                    }

                    if let Some(class_name) = obj_class_name {
                        let (class_def_clone, method_clone) = {
                            let (class_def, method) = self.registry.resolve_class_method(&class_name, method_name.as_str())
                                .ok_or_else(|| IshError::ExecutionError(format!(
                                    "Method '{}' not found on class '{}' or its base classes.", method_name, class_name
                                )))?;
                            (class_def.clone(), method.clone())
                        };

                        Registry::check_access(&method_clone.access, self.current_class.as_deref(), &class_def_clone.qualified_name)?;

                        let mut evaluated_params = Vec::new();
                        let mut arg_idx = 0;
                        for param in &method_clone.params {
                            if param.is_variadic {
                                let mut variadic_arr = Vec::new();
                                while arg_idx < eval_args.len() {
                                    variadic_arr.push(eval_args[arg_idx].clone());
                                    arg_idx += 1;
                                }
                                let arr_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(variadic_arr));
                                evaluated_params.push((param.name.clone(), IshValue::Reference(arr_ref)));
                            } else {
                                if arg_idx < eval_args.len() {
                                    evaluated_params.push((param.name.clone(), eval_args[arg_idx].clone()));
                                    arg_idx += 1;
                                } else {
                                    if let Some(default_expr) = &param.default_value {
                                        let val = self.evaluate_node(default_expr, jobs)?;
                                        evaluated_params.push((param.name.clone(), val));
                                    } else {
                                        evaluated_params.push((param.name.clone(), IshValue::Null));
                                    }
                                }
                            }
                        }

                        self.variables.push(HashMap::new());
                        self.function_bases.push(self.variables.len() - 1);
                        if let Some(scope) = self.variables.last_mut() {
                            scope.insert("this".to_string(), IshValue::Reference(id));
                            for (k, v) in evaluated_params {
                                scope.insert(k, v);
                            }
                        }

                        let prev_class = self.current_class.clone();
                        self.current_class = Some(class_def_clone.qualified_name.clone());
                        let mut success = true;
                        let mut ret_val: Option<IshValue> = None;
                        for stmt in &method_clone.body {
                            success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                            if self.returning { break; }
                        }

                        let ret_val = self.return_value.take();
                        
                        self.returning = false;
                        self.function_bases.pop();
                        let _ = self.pop_scope(jobs);
                        self.current_class = prev_class;

                        if let Some(val) = ret_val {
                            self.return_value = Some(val);
                        }

                        self.last_exit_code = if success { 0 } else { 1 };
                        return Ok(success);
                    }
                }
                
                match obj_val {

                    IshValue::TypeRef(ref s) => {
                        // A. STD-LIB Fallback
                        for provider in &self.stdlib_providers {
                            if provider.name() == s {
                                match provider.execute_method(&method_name, &eval_args, &mut self.gobbler) {
                                    Ok(output_val) => {
                                        self.last_exit_code = 0;
                                        self.return_value = Some(output_val);
                                        return Ok(true);
                                    }
                                    Err(e) => return Err(e),
                                }
                            }
                        }

                        // B. STATIC STRING METHODS
                        if s == "string" {
                            match method_name.as_str() {
                                "IsNullOrWhiteSpace" => {
                                    let is_null_or_ws = match eval_args.get(0) {
                                        Some(IshValue::String(str_val)) => str_val.trim().is_empty(),
                                        Some(IshValue::Null) => true,
                                        None => true,
                                        _ => false,
                                    };
                                    self.return_value = Some(IshValue::Bool(is_null_or_ws));
                                    return Ok(true);
                                }
                                "Join" => {
                                    let arr_val = eval_args.get(0);
                                    let sep_val = eval_args.get(1);
                                    
                                    let sep_char = match sep_val {
                                        Some(IshValue::Char(c)) => c.to_string(),
                                        Some(IshValue::String(s)) => s.clone(),
                                        _ => "".to_string(),
                                    };

                                    let mut strs = Vec::new();
                                    if let Some(IshValue::Reference(id)) = arr_val {
                                        if let Some(heap_obj) = self.gobbler.get(*id) {
                                            match heap_obj {
                                                crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                                                    for item in arr { strs.push(item.to_string()); }
                                                }
                                                _ => return Err(IshError::ExecutionError("string.Join requires an array or list".into())),
                                            }
                                        }
                                    } else {
                                        return Err(IshError::ExecutionError("string.Join requires an array or list".into()));
                                    }
                                    self.return_value = Some(IshValue::String(strs.join(&sep_char)));
                                    return Ok(true);
                                }
                                "Concat" => {
                                    let mut strs = Vec::new();
                                    if eval_args.len() == 1 && matches!(eval_args[0], IshValue::Reference(_)) {
                                        if let IshValue::Reference(id) = eval_args[0] {
                                            if let Some(heap_obj) = self.gobbler.get(id) {
                                                match heap_obj {
                                                    crate::core::gobbler::HeapObject::Array(arr) | crate::core::gobbler::HeapObject::List(arr) => {
                                                        for item in arr { strs.push(item.to_string()); }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    } else {
                                        for arg in eval_args { strs.push(arg.to_string()); }
                                    }
                                    self.return_value = Some(IshValue::String(strs.join("")));
                                    return Ok(true);
                                }
                                _ => return Err(IshError::ExecutionError(format!("Unknown static method on string: {}", method_name))),
                            }
                        } else {
                            Err(IshError::ExecutionError(format!("Cannot call method '{}' on type '{}'", method_name, s)))
                        }
                    }

                    IshValue::String(ref s) => {
                        match method_name.as_str() {
                            "Substring" => {
                                let start = match eval_args.get(0) { Some(IshValue::Int(i)) => *i as usize, _ => 0 };
                                let count = match eval_args.get(1) { Some(IshValue::Int(i)) => *i as usize, _ => s.chars().count() - start };
                                self.return_value = Some(IshValue::String(s.chars().skip(start).take(count).collect()));
                                return Ok(true);
                            }
                            "IndexOf" => {
                                let search = match eval_args.get(0) {
                                    Some(IshValue::String(val)) => val.clone(),
                                    Some(IshValue::Char(c)) => c.to_string(),
                                    _ => "".to_string()
                                };
                                let index = s.find(&search).map(|byte_idx| s[..byte_idx].chars().count() as i32).unwrap_or(-1);
                                self.return_value = Some(IshValue::Int(index));
                                return Ok(true);
                            }
                            "Contains" => {
                                let search = match eval_args.get(0) {
                                    Some(IshValue::String(val)) => val.clone(),
                                    Some(IshValue::Char(c)) => c.to_string(),
                                    _ => "".to_string()
                                };
                                self.return_value = Some(IshValue::Bool(s.contains(&search)));
                                return Ok(true);
                            }
                            "ToLower" => {
                                self.return_value = Some(IshValue::String(s.to_lowercase()));
                                return Ok(true);
                            }
                            "ToUpper" => {
                                self.return_value = Some(IshValue::String(s.to_uppercase()));
                                return Ok(true);
                            }
                            "Trim" => {
                                self.return_value = Some(IshValue::String(s.trim().to_string()));
                                return Ok(true);
                            }
                            "RemoveSubstring" => {
                                let search = match eval_args.get(0) {
                                    Some(IshValue::String(val)) => val.clone(),
                                    Some(IshValue::Char(c)) => c.to_string(),
                                    _ => "".to_string()
                                };
                                self.return_value = Some(IshValue::String(s.replace(&search, "")));
                                return Ok(true);
                            }
                            "Length" => {
                                self.return_value = Some(IshValue::Int(s.chars().count() as i32));
                                return Ok(true);
                            }
                            _ => {
                                for provider in &self.stdlib_providers {
                                    if provider.name() == s {
                                        match provider.execute_method(&method_name, &eval_args, &mut self.gobbler) {
                                            Ok(output_val) => {
                                                self.last_exit_code = 0;
                                                self.return_value = Some(output_val);
                                                return Ok(true);
                                            }
                                            Err(e) => return Err(e),
                                        }
                                    }
                                }

                                let resolved = self.registry.resolve_class_method(&s, method_name.as_str())
                                    .map(|(c, m)| (c.clone(), m.clone()));

                                if let Some((class_def_clone, method_clone)) = resolved {

                                    if !method_clone.is_static {
                                        return Err(IshError::ExecutionError(format!(
                                            "Cannot call instance method '{}' without an object instance.", method_name
                                        )));
                                    }

                                    Registry::check_access(&method_clone.access, self.current_class.as_deref(), &class_def_clone.qualified_name)?;

                                    let mut evaluated_params = Vec::new();
                                    let mut arg_idx = 0;
                                    for param in &method_clone.params {
                                        if param.is_variadic {
                                            let mut variadic_arr = Vec::new();
                                            while arg_idx < eval_args.len() {
                                                variadic_arr.push(eval_args[arg_idx].clone());
                                                arg_idx += 1;
                                            }
                                            let arr_ref = self.gobbler.allocate(crate::core::gobbler::HeapObject::Array(variadic_arr));
                                            evaluated_params.push((param.name.clone(), IshValue::Reference(arr_ref)));
                                        } else {
                                            if arg_idx < eval_args.len() {
                                                evaluated_params.push((param.name.clone(), eval_args[arg_idx].clone()));
                                                arg_idx += 1;
                                            } else {
                                                if let Some(default_expr) = &param.default_value {
                                                    let val = self.evaluate_node(default_expr, jobs)?;
                                                    evaluated_params.push((param.name.clone(), val));
                                                } else {
                                                    evaluated_params.push((param.name.clone(), IshValue::Null));
                                                }
                                            }
                                        }
                                    }

                                    self.variables.push(HashMap::new());
                                    self.function_bases.push(self.variables.len() - 1);
                                    if let Some(scope) = self.variables.last_mut() {
                                        for (k, v) in evaluated_params {
                                            scope.insert(k, v);
                                        }
                                    }

                                    let prev_class = self.current_class.clone();
                                    self.current_class = Some(class_def_clone.qualified_name.clone());
                                    let mut success = true;
                                    let mut ret_val: Option<IshValue> = None;
                                    for stmt in &method_clone.body {
                                        success = self.execute_node_with_input(stmt, "", out_file, jobs)?;
                                        if self.returning { break; }
                                    }

                                    let ret_val = self.return_value.take();
                                    self.returning = false;
                                    self.function_bases.pop();
                                    let _ = self.pop_scope(jobs);

                                    self.current_class = prev_class;

                                    if let Some(val) = ret_val {
                                        self.return_value = Some(val);
                                    }

                                    self.last_exit_code = if success { 0 } else { 1 };
                                    return Ok(success)
                                } else {
                                    Err(IshError::ExecutionError(format!(
                                        "Cannot call method '{}' on non-object value '{}'.", method_name, s
                                    )))
                                }
                            }
                        }
                    }

                    IshValue::Char(c) => {
                        match method_name.as_str() {
                            "IsLetter" => {
                                self.return_value = Some(IshValue::Bool(c.is_alphabetic()));
                                return Ok(true);
                            }
                            "IsDigit" => {
                                self.return_value = Some(IshValue::Bool(c.is_ascii_digit()));
                                return Ok(true);
                            }
                            "IsWhiteSpace" => {
                                self.return_value = Some(IshValue::Bool(c.is_whitespace()));
                                return Ok(true);
                            }
                            "IsAlnum" | "IsLetterOrDigit" => {
                                self.return_value = Some(IshValue::Bool(c.is_alphanumeric()));
                                return Ok(true);
                            }
                            "ToLower" => {
                                self.return_value = Some(IshValue::Char(c.to_lowercase().next().unwrap_or(c)));
                                return Ok(true);
                            }
                            "ToUpper" => {
                                self.return_value = Some(IshValue::Char(c.to_uppercase().next().unwrap_or(c)));
                                return Ok(true);
                            }
                            _ => return Err(IshError::ExecutionError(format!("Char has no method '{}'", method_name))),
                        }
                    }

                    _ => {
                        Err(IshError::ExecutionError(format!(
                            "Cannot call method '{}' on non-object value '{}'.", method_name, obj_val.to_string()
                        )))
                    }
                }
            } // Closes AstNodeKind::MethodCall

            AstNodeKind::EnumDecl { .. } => return Ok(true),
            AstNodeKind::WithImport { .. } => return Ok(true),
            AstNodeKind::KeyValuePair { .. } => return Err(IshError::ExecutionError("KeyValuePair cannot be executed directly".to_string())),
        }
    }
}