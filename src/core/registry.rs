use std::collections::HashMap;
use crate::core::ast::{AccessSpecifier, AstNode, AstNodeKind};
use crate::error::IshError;

/// Represents a registered class definition in the Ish OOP system.
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: String,
    pub qualified_name: String,
    pub access: AccessSpecifier,
    pub is_static: bool,
    pub methods: HashMap<String, MethodDef>,
    pub fields: HashMap<String, FieldDef>,
    pub constructor: Option<(AccessSpecifier, Vec<crate::core::ast::Param>, Vec<AstNode>)>,
    pub destructor: Option<Vec<AstNode>>,
    pub base_class: Option<String>,
}

/// Represents a registered struct definition in the Ish OOP system.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub qualified_name: String,
    pub access: AccessSpecifier,
    pub fields: HashMap<String, FieldDef>,
    pub constructor: Option<(AccessSpecifier, Vec<crate::core::ast::Param>, Vec<AstNode>)>,
    pub destructor: Option<Vec<AstNode>>,
}

/// Represents a method within a class.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: String,
    pub access: AccessSpecifier,
    pub is_static: bool,
    pub params: Vec<crate::core::ast::Param>,
    pub body: Vec<AstNode>,
}

/// Represents a field declaration within a class or struct.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub type_specifier: Option<String>,
    pub access: AccessSpecifier,
    pub is_static: bool,
    pub default_value: Option<AstNode>,
}

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub qualified_name: String,
    pub access: AccessSpecifier,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub classes: HashMap<String, ClassDef>,
    pub structs: HashMap<String, StructDef>,
    pub enums: HashMap<String, EnumDef>,
    namespace_stack: Vec<String>,
    pub loaded_files: HashSet<String>,
}

impl Registry {
    pub fn new() -> Self {
        let mut structs = HashMap::new();
        
        let mut pair_fields = HashMap::new();
        pair_fields.insert("Key".to_string(), FieldDef {
            name: "Key".to_string(),
            type_specifier: None,
            access: AccessSpecifier::Public,
            is_static: false,
            default_value: None,
        });
        pair_fields.insert("Value".to_string(), FieldDef {
            name: "Value".to_string(),
            type_specifier: None,
            access: AccessSpecifier::Public,
            is_static: false,
            default_value: None,
        });
        
        let pair_constructor = (
            AccessSpecifier::Public,
            vec![
                crate::core::ast::Param { name: "k".to_string(), type_specifier: None, is_array: false, is_variadic: false, default_value: None },
                crate::core::ast::Param { name: "v".to_string(), type_specifier: None, is_array: false, is_variadic: false, default_value: None }
            ],
            vec![
                AstNode::new(
                    AstNodeKind::Assignment {
                        type_specifier: None,
                        variable: "this.Key".to_string(),
                        index: None,
                        value: Box::new(AstNode::new(
                            AstNodeKind::Variable("k".to_string()),
                            0, 0
                        )),
                        is_declaration: false,
                    },
                    0, 0
                ),
                AstNode::new(
                    AstNodeKind::Assignment {
                        type_specifier: None,
                        variable: "this.Value".to_string(),
                        index: None,
                        value: Box::new(AstNode::new(
                            AstNodeKind::Variable("v".to_string()),
                            0, 0
                        )),
                        is_declaration: false,
                    },
                    0, 0
                )
            ]
        );

        structs.insert("Pair".to_string(), StructDef {
            name: "Pair".to_string(),
            qualified_name: "Pair".to_string(),
            access: AccessSpecifier::Public,
            fields: pair_fields,
            constructor: Some(pair_constructor),
            destructor: None,
        });

        Self {
            classes: HashMap::new(),
            structs,
            enums: HashMap::new(),
            namespace_stack: Vec::new(),
            loaded_files: HashSet::new(),
        }
    }

    /// Returns the current fully-qualified namespace prefix (e.g. "Ns1::Ns2").
    fn current_namespace(&self) -> String {
        let parts: Vec<String> = self.namespace_stack.iter().filter(|s| !s.is_empty()).cloned().collect();
        parts.join("::")
    }

    /// Builds a fully-qualified name by prepending the current namespace.
    fn qualify_name(&self, name: &str) -> String {
        let ns = self.current_namespace();
        if ns.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", ns, name)
        }
    }

    /// First pass: Walk the AST and register all namespaces, classes, and structs.
    /// This does NOT execute any code — it only populates the registry.
    pub fn register_declarations(&mut self, node: &AstNode) -> Result<(), IshError> {
        match &node.kind {
            AstNodeKind::NamespaceDecl { name, body } => {
                self.namespace_stack.push(name.clone());
                for stmt in body {
                    self.register_declarations(stmt)?;
                }
                self.namespace_stack.pop();
            }
            AstNodeKind::ClassDecl { name, base_class, access, is_static, methods, fields, constructor, destructor } => {
                let qualified_name = self.qualify_name(name);
                let mut method_map = HashMap::new();
                let mut field_map = HashMap::new();

                for method_node in methods {
                    if let AstNodeKind::Function { name: mname, params, body, access: maccess, is_static: mstatic } = &method_node.kind {
                        method_map.insert(mname.clone(), MethodDef {
                            name: mname.clone(),
                            access: maccess.clone(),
                            is_static: *mstatic,
                            params: params.clone(),
                            body: body.clone(),
                        });
                    }
                }

                for field_node in fields {
                    if let AstNodeKind::FieldDecl { name: field_name, type_specifier, access, is_static, default_value } = &field_node.kind {
                        field_map.insert(field_name.clone(), FieldDef {
                            name: field_name.clone(),
                            type_specifier: type_specifier.clone(),
                            access: access.clone(),
                            is_static: *is_static,
                            default_value: default_value.as_ref().map(|b| *b.clone()),
                        });
                    }
                }

                self.classes.insert(qualified_name.clone(), ClassDef {
                    name: name.clone(),
                    qualified_name,
                    access: access.clone(),
                    is_static: *is_static,
                    methods: method_map,
                    fields: field_map,
                    constructor: constructor.clone(),
                    destructor: destructor.clone(),
                    base_class: base_class.clone(),
                });
            }
            AstNodeKind::StructDecl { name, access, fields, constructor, destructor } => {
                let qualified_name = self.qualify_name(name);
                let mut field_map = HashMap::new();

                for field_node in fields {
                    if let AstNodeKind::FieldDecl { name: field_name, type_specifier, access, is_static, default_value } = &field_node.kind {
                        field_map.insert(field_name.clone(), FieldDef {
                            name: field_name.clone(),
                            type_specifier: type_specifier.clone(),
                            access: access.clone(),
                            is_static: *is_static,
                            default_value: default_value.as_ref().map(|b| *b.clone()),
                        });
                    }
                }

                self.structs.insert(qualified_name.clone(), StructDef {
                    name: name.clone(),
                    qualified_name,
                    access: access.clone(),
                    fields: field_map,
                    constructor: constructor.clone(),
                    destructor: destructor.clone(),
                });
            }
            AstNodeKind::EnumDecl { name, access, variants } => {
                let qualified_name = if self.namespace_stack.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", self.current_namespace(), name)
                };
                self.enums.insert(name.clone(), EnumDef {
                    name: name.clone(),
                    qualified_name,
                    access: access.clone(),
                    variants: variants.clone(),
                });
            }

AstNodeKind::WithImport { path } => {
                let target_namespace = path; // e.g., "Mod"
                let base_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

                // 1. Gather ALL .ish files recursively in the project directory
                let mut dirs_to_check = vec![base_dir];
                let mut all_ish_files = Vec::new();

                while let Some(dir) = dirs_to_check.pop() {
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let entry_path = entry.path();
                            if entry_path.is_dir() {
                                dirs_to_check.push(entry_path);
                            } else if entry_path.extension().and_then(|s| s.to_str()) == Some("ish") {
                                all_ish_files.push(entry_path);
                            }
                        }
                    }
                }

                let mut found_namespace = false;

                // 2. Helper function to recursively search the single AstNode for the namespace
                fn node_contains_namespace(node: &AstNode, target: &str) -> bool {
                    match &node.kind {
                        AstNodeKind::NamespaceDecl { name, body } => {
                            if name.as_str() == target {
                                return true;
                            }
                            for stmt in body {
                                if node_contains_namespace(stmt, target) {
                                    return true;
                                }
                            }
                        }
                        AstNodeKind::If { body, else_body, .. } => {
                            for stmt in body {
                                if node_contains_namespace(stmt, target) {
                                    return true;
                                }
                            }
                            if let Some(else_stmts) = else_body {
                                for stmt in else_stmts {
                                    if node_contains_namespace(stmt, target) {
                                        return true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    false
                }

                // 3. Scan every file to see if it belongs to the requested namespace
                for absolute_path in all_ish_files {
                    let absolute_path_str = absolute_path.to_string_lossy().to_string();
                    
                    if self.loaded_files.contains(&absolute_path_str) {
                        continue;
                    }

                    if let Ok(content) = std::fs::read_to_string(&absolute_path) {
                        let mut tokenizer = crate::core::tokenizer::Tokenizer::new(&content);
                        if let Ok(tokens) = tokenizer.tokenize() {
                            let mut parser = crate::core::parser::Parser::new(tokens);
                            if let Ok(ast) = parser.parse() {
                                
                                // Call our recursive helper instead of using a loop on &ast
                                if node_contains_namespace(&ast, target_namespace.as_str()) {
                                    self.loaded_files.insert(absolute_path_str);
                                    let _ = self.register_declarations(&ast);
                                    found_namespace = true;
                                }
                            }
                        }
                    }
                }

                if !found_namespace {
                    eprintln!("Error: Could not find any files containing namespace '{}'", target_namespace);
                }
            }
            
            AstNodeKind::If { body, else_body, .. } => {
                for stmt in body {
                    self.register_declarations(stmt)?;
                }
                if let Some(else_stmts) = else_body {
                    for stmt in else_stmts {
                        self.register_declarations(stmt)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn resolve_class(&self, name: &str) -> Option<&ClassDef> {
        if let Some(cls) = self.classes.get(name) {
            return Some(cls);
        }
        for (qn, cls) in &self.classes {
            if qn.ends_with(&format!("::{}", name)) || qn == name {
                return Some(cls);
            }
        }
        None
    }

    pub fn resolve_class_method<'a>(&'a self, start_class: &str, method_name: &str) -> Option<(&'a ClassDef, &'a MethodDef)> {
        let mut current_class_name = start_class.to_string();
        loop {
            if let Some(class_def) = self.resolve_class(&current_class_name) {
                if let Some(method) = class_def.methods.get(method_name) {
                    return Some((class_def, method));
                }
                if let Some(ref base) = class_def.base_class {
                    current_class_name = base.clone();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        None
    }

    pub fn resolve_struct(&self, name: &str) -> Option<&StructDef> {
        if let Some(s) = self.structs.get(name) {
            return Some(s);
        }
        for (qn, s) in &self.structs {
            if qn.ends_with(&format!("::{}", name)) || qn == name {
                return Some(s);
            }
        }
        None
    }

    pub fn find_entry_point(&self, entry_class: &str, entry_method: &str, with_args: bool) -> Result<String, IshError> {
        for (qn, cls) in &self.classes {
            if cls.name == entry_class {
                if !cls.is_static {
                    return Err(IshError::ParseError(format!(
                        "Entry point class '{}' must be declared as 'static'.", qn
                    )));
                }
                if let Some(AccessSpecifier::Public) = Some(&cls.access) {
                    // Valid access
                } else {
                    return Err(IshError::ParseError(format!(
                        "Entry point class '{}' must be declared as 'public'.", qn
                    )));
                }
                if let Some(method) = cls.methods.get(entry_method) {
                    if !method.is_static {
                        return Err(IshError::ParseError(
                            format!("Entry point method '{}' must be declared as 'static'.", entry_method)
                        ));
                    }
                    if !matches!(method.access, AccessSpecifier::Public) {
                        return Err(IshError::ParseError(
                            format!("Entry point method '{}' must be declared as 'public'.", entry_method)
                        ));
                    }

                    if with_args {
                        if !method.params.last().map_or(false, |p| p.is_variadic && p.type_specifier.as_deref() == Some("string[]")) {
                            return Err(IshError::ParseError(
                                format!("Entry point method '{}' must have the 'params string[] args' signature.", entry_method)
                            ));
                        }
                    }
                    return Ok(qn.clone());
                } else {
                    return Err(IshError::ParseError(format!(
                        "Entry point class '{}' must contain a 'public static func {}' method.", qn, entry_method
                    )));
                }
            }
        }
        Err(IshError::ParseError(format!("Entry point class '{}' not found.", entry_class)))
    }

    pub fn check_access(access: &AccessSpecifier, caller_class: Option<&str>, target_class: &str) -> Result<(), IshError> {
        match access {
            AccessSpecifier::Public => Ok(()),
            AccessSpecifier::Internal => Ok(()),
            AccessSpecifier::Protected => {
                if caller_class == Some(target_class) {
                    Ok(())
                } else {
                    Err(IshError::ExecutionError(format!(
                        "Cannot access protected member of '{}' from outside its class hierarchy.", target_class
                    )))
                }
            }
            AccessSpecifier::Private => {
                if caller_class == Some(target_class) {
                    Ok(())
                } else {
                    Err(IshError::ExecutionError(format!(
                        "Cannot access private member of '{}' from outside its class.", target_class
                    )))
                }
            }
        }
    }
}