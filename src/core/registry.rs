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
}

/// Represents a registered struct definition in the Ish OOP system.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub qualified_name: String,
    pub access: AccessSpecifier,
    pub fields: HashMap<String, FieldDef>,
}

/// Represents a method within a class.
#[derive(Debug, Clone)]
pub struct MethodDef {
    pub name: String,
    pub access: AccessSpecifier,
    pub is_static: bool,
    pub params: Vec<String>,
    pub body: Vec<AstNode>,
}

/// Represents a field declaration within a class or struct.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub access: AccessSpecifier,
    pub default_value: Option<AstNode>,
}

/// The OOP registry that holds all namespace, class, and struct definitions.
/// It is populated during the "first pass" of AST traversal before execution begins.
#[derive(Debug, Clone)]
pub struct Registry {
    /// Map of fully-qualified class name -> ClassDef (e.g. "MyNamespace::MyClass")
    pub classes: HashMap<String, ClassDef>,
    /// Map of fully-qualified struct name -> StructDef
    pub structs: HashMap<String, StructDef>,
    /// Tracks the current namespace prefix stack during registration
    namespace_stack: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            classes: HashMap::new(),
            structs: HashMap::new(),
            namespace_stack: Vec::new(),
        }
    }

    /// Returns the current fully-qualified namespace prefix (e.g. "Ns1::Ns2").
    fn current_namespace(&self) -> String {
        self.namespace_stack.join("::")
    }

    /// Builds a fully-qualified name by prepending the current namespace.
    fn qualify_name(&self, name: &str) -> String {
        if self.namespace_stack.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", self.current_namespace(), name)
        }
    }

    /// First pass: Walk the AST and register all namespaces, classes, and structs.
    /// This does NOT execute any code — it only populates the registry.
    pub fn register_declarations(&mut self, node: &AstNode) -> Result<(), IshError> {
        match &node.kind {
            AstNodeKind::Sequential(left, right) => {
                self.register_declarations(left)?;
                self.register_declarations(right)?;
            }
            AstNodeKind::NamespaceDecl { name, body } => {
                self.namespace_stack.push(name.clone());
                for stmt in body {
                    self.register_declarations(stmt)?;
                }
                self.namespace_stack.pop();
            }
            AstNodeKind::ClassDecl { name, access, is_static, methods, fields } => {
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
                    if let AstNodeKind::Assignment { variable, value, .. } = &field_node.kind {
                        field_map.insert(variable.clone(), FieldDef {
                            name: variable.clone(),
                            access: access.clone(),
                            default_value: Some(*value.clone()),
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
                });
            }
            AstNodeKind::StructDecl { name, access, fields } => {
                let qualified_name = self.qualify_name(name);
                let mut field_map = HashMap::new();

                for field_node in fields {
                    if let AstNodeKind::Assignment { variable, value, .. } = &field_node.kind {
                        field_map.insert(variable.clone(), FieldDef {
                            name: variable.clone(),
                            access: access.clone(),
                            default_value: Some(*value.clone()),
                        });
                    }
                }

                self.structs.insert(qualified_name.clone(), StructDef {
                    name: name.clone(),
                    qualified_name,
                    access: access.clone(),
                    fields: field_map,
                });
            }
            // Recurse into other compound nodes that might contain declarations
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
            // All other node kinds are not declarations; skip them
            _ => {}
        }
        Ok(())
    }

    /// Look up a class by name, trying both bare name and qualified name.
    pub fn resolve_class(&self, name: &str) -> Option<&ClassDef> {
        // Try exact match first
        if let Some(cls) = self.classes.get(name) {
            return Some(cls);
        }
        // Try all qualified names that end with ::name
        for (qn, cls) in &self.classes {
            if qn.ends_with(&format!("::{}", name)) || qn == name {
                return Some(cls);
            }
        }
        None
    }

    /// Look up a struct by name, trying both bare name and qualified name.
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

    /// Validate that a `Program` class with a `main` method exists.
    /// Returns the qualified name of the Program class.
    pub fn find_entry_point(&self) -> Result<String, IshError> {
        for (qn, cls) in &self.classes {
            if cls.name == "Program" {
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
                if let Some(main_method) = cls.methods.get("main") {
                    if !main_method.is_static {
                        return Err(IshError::ParseError(
                            "Entry point method 'main' must be declared as 'static'.".to_string()
                        ));
                    }
                    if !matches!(main_method.access, AccessSpecifier::Public) {
                        return Err(IshError::ParseError(
                            "Entry point method 'main' must be declared as 'public'.".to_string()
                        ));
                    }
                    return Ok(qn.clone());
                } else {
                    return Err(IshError::ParseError(format!(
                        "Entry point class '{}' must contain a 'public static func main()' method.", qn
                    )));
                }
            }
        }
        Err(IshError::ParseError(
            "No entry point found. Scripts must contain a 'public static class Program' with a 'public static func main()' method.".to_string()
        ))
    }

    /// Check if an access specifier allows access from outside the class.
    pub fn check_access(access: &AccessSpecifier, caller_class: Option<&str>, target_class: &str) -> Result<(), IshError> {
        match access {
            AccessSpecifier::Public => Ok(()),
            AccessSpecifier::Internal => Ok(()), // Within same assembly (for now, always allowed)
            AccessSpecifier::Protected => {
                // For now, only allow within same class (no inheritance yet)
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
