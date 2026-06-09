#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Word(String),
    Variable(String),
    StringLiteral(String),

    // Ish custom syntax
    Pipe,           // :
    RedirectTo,     // to
    RedirectFrom,   // from
    AppendTo,       // append to
    ReadDoc,        // read doc
    MergeErr,       // merge err
    DevNull,        // DevNull
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

    // Scripting keywords
    If,
    Elif,
    Else,
    For,
    Foreach,
    Let,
    WhileLoop,      // Note: 'while' can mean parallel exec or while loop. We disambiguate in Parser.
    Function,
    Return,
    Break,
    Continue,

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

            let start_line = self.line;
            let start_column = self.column;
            let ch = self.input[self.position];

            let kind = match ch {
                ':' | '|' => {
                    if ch == '|' && self.position + 1 < self.input.len() && self.input[self.position + 1] == '|' {
                        self.advance();
                        self.advance();
                        TokenKind::OrElse
                    } else {
                        self.advance();
                        TokenKind::Pipe
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
                ',' => {
                    self.advance();
                    TokenKind::Comma
                }
                '$' => {
                    self.advance();
                    let var_name = self.read_word();
                    TokenKind::Variable(var_name)
                }
                '"' | '\'' => {
                    let string_val = self.read_string(ch)?;
                    TokenKind::StringLiteral(string_val)
                }
                _ => {
                    let word = self.read_word();
                    if word.is_empty() {
                        self.advance();
                        continue;
                    }

                    if word == "and" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "then" {
                            self.read_word();
                            TokenKind::AndThen
                        } else {
                            TokenKind::Word(word)
                        }
                    } else if word == "or" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "else" {
                            self.read_word();
                            TokenKind::OrElse
                        } else {
                            TokenKind::Word(word)
                        }
                    } else if word == "append" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "to" {
                            self.read_word();
                            TokenKind::AppendTo
                        } else {
                            TokenKind::Word(word)
                        }
                    } else if word == "read" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "doc" {
                            self.read_word();
                            TokenKind::ReadDoc
                        } else {
                            TokenKind::Word(word)
                        }
                    } else if word == "merge" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "err" {
                            self.read_word();
                            TokenKind::MergeErr
                        } else {
                            TokenKind::Word(word)
                        }
                    } else {
                        match word.as_str() {
                            "to" => TokenKind::RedirectTo,
                            "from" => TokenKind::RedirectFrom,
                            "DevNull" => TokenKind::DevNull,
                            "then" => TokenKind::Then,
                            "while" => TokenKind::WhileAsync,
                            "job" => TokenKind::Job,
                            "if" => TokenKind::If,
                            "elif" => TokenKind::Elif,
                            "else" => TokenKind::Else,
                            "for" => TokenKind::For,
                            "foreach" => TokenKind::Foreach,
                            "let" => TokenKind::Let,
                            "fn" => TokenKind::Function,
                            "return" => TokenKind::Return,
                            "break" => TokenKind::Break,
                            "continue" => TokenKind::Continue,
                            _ => TokenKind::Word(word),
                        }
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
            if ch.is_whitespace() || "{}()=,:\"'$!<>".contains(ch) {
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
}
