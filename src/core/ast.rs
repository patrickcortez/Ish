#[derive(Clone, Debug, PartialEq)]
pub enum IshValue {
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Char(char),
    Null,
    Reference(usize),
    TypeRef(String)
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
    pub type_specifier: Option<String>,
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
            IshValue::Char(c) => c.to_string(),
            IshValue::Reference(id) => format!("<Reference {}>", id),
            IshValue::TypeRef(t) => format!("<Type {}>", t),
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
    /// String Literal
    StringLiteral(String),

    CharLiteral(char),

    /// Variable Reference
    Variable(String),

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
        type_specifier: Option<String>,
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
        base_class: Option<String>,
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

    /// OOP: Field Declaration
    FieldDecl {
        name: String,
        type_specifier: Option<String>,
        access: AccessSpecifier,
        is_static: bool,
        default_value: Option<Box<AstNode>>,
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

    /// OOP: Index Access (`$obj[0]`)
    IndexAccess {
        object: Box<AstNode>,
        index: Box<AstNode>,
    },

    /// Switch Statement
    Switch {
        expression: Box<AstNode>,
        cases: Vec<(AstNode, Vec<AstNode>)>,
        default_case: Option<Vec<AstNode>>,
    },

    /// Interpolated String (`$"value is {val}"`)
    InterpolatedString(Vec<AstNode>),
}