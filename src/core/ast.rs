use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum IshValue {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Array(Vec<IshValue>),
    List(Vec<IshValue>),
    Map(HashMap<String, IshValue>),
    Object {
        class_name: String,
        properties: HashMap<String, IshValue>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessSpecifier {
    Public,
    Private,
    Protected,
    Internal,
}

impl IshValue {
    pub fn to_string(&self) -> String {
        match self {
            IshValue::String(s) => s.clone(),
            IshValue::Int(i) => i.to_string(),
            IshValue::Float(f) => f.to_string(),
            IshValue::Bool(b) => b.to_string(),
            IshValue::Null => "null".to_string(),
            IshValue::Array(arr) | IshValue::List(arr) => {
                // If the array contains ONLY maps, we can print it as a tabulated table.
                if !arr.is_empty() && arr.iter().all(|v| matches!(v, IshValue::Map(_))) {
                    self.to_table_string()
                } else {
                    let s: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                    format!("[{}]", s.join(", "))
                }
            }
            IshValue::Map(m) => {
                let mut s: Vec<String> = m.iter().map(|(k, v)| format!("{}: {}", k, v.to_string())).collect();
                s.sort();
                format!("{{{}}}", s.join(", "))
            }
            IshValue::Object { class_name, properties: _ } => {
                format!("<Object {}>", class_name)
            }
        }
    }

    pub fn to_table_string(&self) -> String {
        match self {
            IshValue::Array(arr) | IshValue::List(arr) => {
                if arr.is_empty() { return "[]".to_string(); }
                // Collect all unique keys from all maps
                let mut keys: Vec<String> = Vec::new();
                for val in arr {
                    if let IshValue::Map(m) = val {
                        for k in m.keys() {
                            if !keys.contains(k) {
                                keys.push(k.clone());
                            }
                        }
                    }
                }
                keys.sort();

                if keys.is_empty() { return "[]".to_string(); }

                let mut table = comfy_table::Table::new();
                table.load_preset(comfy_table::presets::UTF8_FULL);
                
                table.set_header(keys.clone());

                // Rows
                for val in arr {
                    if let IshValue::Map(m) = val {
                        let mut row = Vec::new();
                        for k in &keys {
                            let str_val = m.get(k).map(|v| v.to_string()).unwrap_or_default();
                            row.push(str_val);
                        }
                        table.add_row(row);
                    }
                }

                table.to_string() + "\n"
            }
            IshValue::Map(m) => {
                let mut keys: Vec<String> = m.keys().cloned().collect();
                keys.sort();
                let mut max_key_len = 0;
                for k in &keys { if k.len() > max_key_len { max_key_len = k.len(); } }

                let mut out = String::new();
                for k in keys {
                    let v = &m[&k];
                    out.push_str(&format!(" {:<width$} | {}\n", k, v.to_string(), width=max_key_len));
                }
                out
            }
            _ => self.to_string(),
        }
    }
}

impl From<serde_json::Value> for IshValue {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => IshValue::Null,
            serde_json::Value::Bool(b) => IshValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    IshValue::Int(i as i32)
                } else if let Some(f) = n.as_f64() {
                    IshValue::Float(f as f32)
                } else {
                    IshValue::String(n.to_string())
                }
            }
            serde_json::Value::String(s) => IshValue::String(s),
            serde_json::Value::Array(arr) => {
                let vec: Vec<IshValue> = arr.into_iter().map(|v| IshValue::from(v)).collect();
                IshValue::Array(vec)
            }
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, val) in obj {
                    map.insert(k, IshValue::from(val));
                }
                IshValue::Map(map)
            }
        }
    }
}

impl Into<serde_json::Value> for IshValue {
    fn into(self) -> serde_json::Value {
        match self {
            IshValue::String(s) => serde_json::Value::String(s),
            IshValue::Int(i) => serde_json::Value::Number(serde_json::Number::from(i)),
            IshValue::Float(f) => {
                if let Some(n) = serde_json::Number::from_f64(f as f64) {
                    serde_json::Value::Number(n)
                } else {
                    serde_json::Value::Null
                }
            }
            IshValue::Bool(b) => serde_json::Value::Bool(b),
            IshValue::Null => serde_json::Value::Null,
            IshValue::Array(arr) | IshValue::List(arr) => {
                let vec: Vec<serde_json::Value> = arr.into_iter().map(|v| v.into()).collect();
                serde_json::Value::Array(vec)
            }
            IshValue::Map(m) => {
                let mut obj = serde_json::Map::new();
                for (k, val) in m {
                    obj.insert(k, val.into());
                }
                serde_json::Value::Object(obj)
            }
            IshValue::Object { class_name, properties } => {
                let mut obj = serde_json::Map::new();
                obj.insert("__class".to_string(), serde_json::Value::String(class_name));
                for (k, val) in properties {
                    obj.insert(k, val.into());
                }
                serde_json::Value::Object(obj)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstNode {
    pub kind: AstNodeKind,
    pub line: usize,
    pub column: usize,
}

impl AstNode {
    pub fn new(kind: AstNodeKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstNodeKind {
    /// A basic command (e.g., `ls -l`)
    Command {
        program: String,
        args: Vec<String>,
        redirect_to: Option<String>,
        redirect_from: Option<String>,
        append_to: Option<String>,
        read_doc: Option<String>,
        merge_err: bool,
    },

    /// Piped commands (e.g., `cmd1 : cmd2 : cmd3`)
    Pipeline(Vec<AstNode>),

    /// Sequential execution (e.g., `cmd1 then cmd2`)
    Sequential(Box<AstNode>, Box<AstNode>),

    /// Logical AND (e.g., `cmd1 and then cmd2`)
    AndThen(Box<AstNode>, Box<AstNode>),

    /// Logical OR (e.g., `cmd1 or else cmd2`)
    OrElse(Box<AstNode>, Box<AstNode>),

    /// Parallel execution (e.g., `cmd1 while cmd2`)
    Parallel(Box<AstNode>, Box<AstNode>),

    /// Background execution (e.g., `cmd job`)
    Background(Box<AstNode>),

    /// String Literal
    StringLiteral(String),

    /// If conditional
    If {
        condition: Box<AstNode>,
        body: Vec<AstNode>,
        else_body: Option<Vec<AstNode>>,
    },

    /// For loop (simplified)
    For {
        variable: String,
        iterable: Box<AstNode>, // e.g., a command that outputs a list
        body: Vec<AstNode>,
    },

    /// Variable assignment
    Assignment {
        variable: String,
        index: Option<Box<AstNode>>,
        value: Box<AstNode>,
        is_declaration: bool,
    },

    /// Binary Condition (e.g. `$a == "test"`)
    Condition {
        left: Box<AstNode>,
        operator: String,
        right: Box<AstNode>,
    },



    /// While loop
    While {
        condition: Box<AstNode>,
        body: Vec<AstNode>,
    },

    /// Function definition
    Function {
        name: String,
        params: Vec<String>,
        has_params: bool, // For `params let[]` support
        body: Vec<AstNode>,
        access: AccessSpecifier,
        is_static: bool,
    },

    /// Try-Catch block
    TryCatch {
        try_body: Vec<AstNode>,
        error_var: String,
        catch_body: Vec<AstNode>,
    },

    /// Subshell execution that returns a value (e.g. `$(...)`)
    Subshell(Box<AstNode>),

    /// Return statement
    Return(Box<AstNode>),

    Break,

    Continue,

    Array(Vec<AstNode>),
    
    Map(Vec<(String, AstNode)>),

    /// OOP: Namespace Declaration
    NamespaceDecl {
        name: String,
        body: Vec<AstNode>,
    },

    /// OOP: Class Declaration
    ClassDecl {
        name: String,
        access: AccessSpecifier,
        is_static: bool,
        methods: Vec<AstNode>,
        fields: Vec<AstNode>,
        constructor: Option<Vec<AstNode>>,
        destructor: Option<Vec<AstNode>>,
    },

    /// OOP: Struct Declaration
    StructDecl {
        name: String,
        access: AccessSpecifier,
        fields: Vec<AstNode>,
    },

    /// OOP: Enum Declaration
    EnumDecl {
        name: String,
        access: AccessSpecifier,
        variants: Vec<String>,
    },

    /// OOP: Namespace/File Import
    WithImport {
        path: String,
    },

    /// OOP: Object Instantiation (`new ClassName()`)
    ObjectInstantiation {
        class_name: String,
        args: Vec<AstNode>,
    },

    /// OOP: Method Call (`$obj.method()`)
    MethodCall {
        object: Box<AstNode>,
        method_name: String,
        args: Vec<AstNode>,
    },

    /// OOP: Property Access (`$obj.field`)
    PropertyAccess {
        object: Box<AstNode>,
        property_name: String,
    },
}
