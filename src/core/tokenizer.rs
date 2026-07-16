#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Word(String),
    StringLiteral(String),
    CharLiteral(char),
    InterpolatedStringLiteral(String),

    // Ish custom syntax
    Colon,          // :
    Then,           // then
    WhileAsync,     // while
    AndThen,        // and then
    OrElse,         // or else
    Job,            // job

    // Operators
    Equals,         // ==
    NotEquals,      // !=
    GreaterThan,    // >
    LessThan,       // <
    GreaterOrEq,    // >=
    LessOrEq,       // <=

    Question,       // ?

    // Scripting keywords
    If,
    Elif,
    Else,
    For,
    Foreach,
    Var,
    StringKeyword,
    IntKeyword,
    BoolKeyword,
    FloatKeyword,
    CharKeyWord,
    WhileLoop,      // Note: 'while' can mean parallel exec or while loop. We disambiguate in Parser.
    Function,
    Return,
    Break,
    Continue,
    Try,
    Catch,
    Switch,
    Case,
    Default,
    
    // OOP Keywords
    Class,
    Struct,
    Namespace,
    Async,
    Await,
    Task,
    Thread,
    Public,
    Private,
    Protected,
    Internal,
    Static,
    New,
    With,
    Enum,
    Params,
    List,

    // Symbols
    LBrace,         // {
    RBrace,         // }
    LParen,         // (
    RParen,         // )
    LBracket,       // [
    RBracket,       // ]
    Assign,         // =
    Comma,          // ,
    Semicolon,      // ; (Used in some for loops perhaps, though 'then' replaces it generally)
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

pub struct Tokenizer {
    input: Vec<char>,
    position: usize,
    line: usize,
    column: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
            line: 1,
            column: 1,
        }
    }

    fn advance(&mut self) -> char {
        let ch = self.input[self.position];
        self.position += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        ch
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, crate::error::IshError> {
        let mut tokens = Vec::new();

        while self.position < self.input.len() {
            self.skip_whitespace();
            if self.position >= self.input.len() {
                break;
            }

            if self.input[self.position] == '#' || (self.input[self.position] == '/' && self.position + 1 < self.input.len() && self.input[self.position + 1] == '/') {
                while self.position < self.input.len() && self.input[self.position] != '\n' {
                    self.advance();
                }
                continue;
            }

            let start_line = self.line;
            let start_column = self.column;
            let ch = self.input[self.position];

            let kind = match ch {
                ':' => {
                    self.advance();
                    TokenKind::Colon
                }
                '|' => {
                    if self.position + 1 < self.input.len() && self.input[self.position + 1] == '|' {
                        self.advance();
                        self.advance();
                        TokenKind::OrElse
                    } else {
                        self.advance();
                        TokenKind::Word("|".to_string())
                    }
                }
                '&' => {
                    if self.position + 1 < self.input.len() && self.input[self.position + 1] == '&' {
                        self.advance();
                        self.advance();
                        TokenKind::AndThen
                    } else {
                        self.advance();
                        TokenKind::Word("&".to_string())
                    }
                }
                '{' => {
                    self.advance();
                    TokenKind::LBrace
                }
                '}' => {
                    self.advance();
                    TokenKind::RBrace
                }
                '(' => {
                    self.advance();
                    TokenKind::LParen
                }
                ')' => {
                    self.advance();
                    TokenKind::RParen
                }
                '[' => {
                    self.advance();
                    TokenKind::LBracket
                }
                ']' => {
                    self.advance();
                    TokenKind::RBracket
                }

                '\n' => {
                    self.advance();
                    TokenKind::Semicolon
                }
                '=' => {
                    self.advance();
                    if self.position < self.input.len() && self.input[self.position] == '=' {
                        self.advance();
                        TokenKind::Equals
                    } else {
                        TokenKind::Assign
                    }
                }
                '!' => {
                    self.advance();
                    if self.position < self.input.len() && self.input[self.position] == '=' {
                        self.advance();
                        TokenKind::NotEquals
                    } else {
                        TokenKind::Word("!".to_string())
                    }
                }
                '>' => {
                    self.advance();
                    if self.position < self.input.len() && self.input[self.position] == '=' {
                        self.advance();
                        TokenKind::GreaterOrEq
                    } else {
                        TokenKind::GreaterThan
                    }
                }
                '<' => {
                    self.advance();
                    if self.position < self.input.len() && self.input[self.position] == '=' {
                        self.advance();
                        TokenKind::LessOrEq
                    } else {
                        TokenKind::LessThan
                    }
                }
                '?' => {
                    self.advance();
                    TokenKind::Question
                }
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '$' => {
                    self.advance();
                    if self.position < self.input.len() && self.input[self.position] == '"' {
                        // Interpolated string handling
                        let str_val = self.read_string('"')?;
                        TokenKind::InterpolatedStringLiteral(str_val)
                    } else {
                        return Err(crate::error::IshError::ParseError("Variables no longer use the $ prefix in Ish.".to_string()));
                    }
                }
                '"' => {
                    let string_val = self.read_string(ch)?;
                    TokenKind::StringLiteral(string_val)
                }
                '\'' => {
                    let char_val = self.read_char_literal()?;
                    TokenKind::CharLiteral(char_val)
                }
                _ => {
                    let word = self.read_word();
                    if word.is_empty() {
                        self.advance();
                        continue;
                    }

                    match word.as_str() {
                        "if" => TokenKind::If,
                        "elif" => TokenKind::Elif,
                        "else" => TokenKind::Else,
                        "while" => TokenKind::WhileAsync,
                        "for" => TokenKind::For,
                        "foreach" => TokenKind::Foreach,
                        "var" => TokenKind::Var,
                        "string" => TokenKind::StringKeyword,
                        "int" => TokenKind::IntKeyword,
                        "bool" => TokenKind::BoolKeyword,
                        "float" => TokenKind::FloatKeyword,
                        "fn" | "func" => TokenKind::Function,
                        "return" => TokenKind::Return,
                        "break" => TokenKind::Break,
                        "continue" => TokenKind::Continue,
                        "try" => TokenKind::Try,
                        "catch" => TokenKind::Catch,
                        "switch" => TokenKind::Switch,
                        "case" => TokenKind::Case,
                        "default" => TokenKind::Default,
                        "class" => TokenKind::Class,
                        "struct" => TokenKind::Struct,
                        "namespace" => TokenKind::Namespace,
                        "async" => TokenKind::Async,
                        "await" => TokenKind::Await,
                        "Task" => TokenKind::Task,
                        "Thread" => TokenKind::Thread,
                        "public" => TokenKind::Public,
                        "private" => TokenKind::Private,
                        "protected" => TokenKind::Protected,
                        "internal" => TokenKind::Internal,
                        "static" => TokenKind::Static,
                        "new" => TokenKind::New,
                        "with" => TokenKind::With,
                        "enum" => TokenKind::Enum,
                        "params" => TokenKind::Params,
                        "List" => TokenKind::List,
                        "char" => TokenKind::CharKeyWord,
                        _ => TokenKind::Word(word),
                    }
                }
            };
            tokens.push(Token {
                kind,
                line: start_line,
                column: start_column,
            });
        }

        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.input[self.position].is_whitespace() && self.input[self.position] != '\n' {
            self.advance();
        }
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while self.position < self.input.len() {
            let ch = self.input[self.position];
            if ch.is_whitespace() || "{}()[]=,:;\"'$!<>?".contains(ch) {
                break;
            }
            word.push(ch);
            self.advance();
        }
        word
    }

    fn peek_word(&mut self) -> String {
        let original_pos = self.position;
        let original_line = self.line;
        let original_col = self.column;
        let word = self.read_word();
        self.position = original_pos;
        self.line = original_line;
        self.column = original_col;
        word
    }

    fn read_string(&mut self, quote: char) -> Result<String, crate::error::IshError> {
        self.advance(); // skip opening quote
        let mut string_val = String::new();
        let mut escaped = false;

        while self.position < self.input.len() {
            let ch = self.input[self.position];
            if escaped {
                string_val.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                self.advance(); // skip closing quote
                return Ok(string_val);
            } else {
                string_val.push(ch);
            }
            self.advance();
        }

        Err(crate::error::IshError::ParseError("Unterminated string".to_string()))
    }

    fn read_char_literal(&mut self) -> Result<char, crate::error::IshError> {
        self.advance(); // Skip opening quote
        let mut char_val = '\0';
        if self.position < self.input.len() {
            let ch = self.input[self.position];
            if ch == '\\' {
                self.advance();
                if self.position < self.input.len() {
                    let next_ch = self.input[self.position];
                    char_val = match next_ch {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        '0' => '\0',
                        _ => next_ch,
                    };
                    self.advance();
                }
            } else {
                char_val = ch;
                self.advance();
            }
        }
        
        if self.position < self.input.len() && self.input[self.position] == '\'' {
            self.advance(); // Skip closing quote
            Ok(char_val)
        } else {
            Err(crate::error::IshError::ParseError("Unterminated char literal".to_string()))
        }
    }
}
