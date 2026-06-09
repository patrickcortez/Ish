use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum IshValue {
    String(String),
    Array(Vec<IshValue>),
    Map(HashMap<String, IshValue>),
}

impl IshValue {
    pub fn to_string(&self) -> String {
        match self {
            IshValue::String(s) => s.clone(),
            IshValue::Array(arr) => {
                let s: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("[{}]", s.join(", "))
            }
            IshValue::Map(m) => {
                let mut s: Vec<String> = m.iter().map(|(k, v)| format!("{}=\"{}\"", k, v.to_string())).collect();
                s.sort();
                format!("Map({})", s.join(", "))
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
        value: Box<AstNode>,
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
        body: Vec<AstNode>,
    },

    /// Return statement
    Return(Box<AstNode>),

    Break,

    Continue,

    Array(Vec<AstNode>),
    
    Map(Vec<(String, AstNode)>),
}
