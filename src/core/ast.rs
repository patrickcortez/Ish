use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum IshValue {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Null,
    Reference(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccessSpecifier {
    Public,
    Private,
    Protected,
    Internal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub is_array: bool,
    pub is_variadic: bool,
    pub default_value: Option<Box<AstNode>>,
}

impl IshValue {
    pub fn to_string(&self) -> String {
        match self {
            IshValue::String(s) => s.clone(),
            IshValue::Int(i) => i.to_string(),
            IshValue::Float(f) => f.to_string(),
            IshValue::Bool(b) => b.to_string(),
            IshValue::Null => "null".to_string(),
            IshValue::Reference(id) => format!("<Reference {}>", id),
        }
    }

    pub fn to_table_string(&self) -> String {
        self.to_string()
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

    /// Binary Operation (e.g. `$a == "test"`, `$a + $b`)
    BinaryOp {
        left: Box<AstNode>,
        operator: String,
        right: Box<AstNode>,
    },

    /// Unary Operation (e.g. `!$a`, `-$b`)
    UnaryOp {
        operator: String,
        operand: Box<AstNode>,
    },

    /// Ternary Operation (e.g. `$a ? $b : $c`)
    TernaryOp {
        condition: Box<AstNode>,
        true_value: Box<AstNode>,
        false_value: Box<AstNode>,
    },    /// While loop
    While {
        condition: Box<AstNode>,
        body: Vec<AstNode>,
    },

    /// Function definition
    Function {
        name: String,
        params: Vec<Param>,
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
        constructor: Option<(AccessSpecifier, Vec<Param>, Vec<AstNode>)>,
        destructor: Option<Vec<AstNode>>,
    },

    /// OOP: Struct Declaration
    StructDecl {
        name: String,
        access: AccessSpecifier,
        fields: Vec<AstNode>,
        constructor: Option<(AccessSpecifier, Vec<Param>, Vec<AstNode>)>,
        destructor: Option<Vec<AstNode>>,
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
