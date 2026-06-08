#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    Word(String),
    Variable(String),

    // Ish custom syntax
    Pipe,           // :
    RedirectTo,     // to
    RedirectFrom,   // from
    Then,           // then
    WhileAsync,     // while
    AndThen,        // and then
    OrElse,         // or else
    Job,            // job

    // Scripting keywords
    If,
    Elif,
    Else,
    For,
    Foreach,
    WhileLoop,      // Note: 'while' can mean parallel exec or while loop. We disambiguate in Parser.
    Function,

    // Symbols
    LBrace,         // {
    RBrace,         // }
    LParen,         // (
    RParen,         // )
    Assign,         // =
    Comma,          // ,
    Semicolon,      // ; (Used in some for loops perhaps, though 'then' replaces it generally)
}

pub struct Tokenizer {
    input: Vec<char>,
    position: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            position: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, crate::error::IshError> {
        let mut tokens = Vec::new();

        while self.position < self.input.len() {
            self.skip_whitespace();
            if self.position >= self.input.len() {
                break;
            }

            let ch = self.input[self.position];

            match ch {
                ':' => {
                    tokens.push(Token::Pipe);
                    self.position += 1;
                }
                '{' => {
                    tokens.push(Token::LBrace);
                    self.position += 1;
                }
                '}' => {
                    tokens.push(Token::RBrace);
                    self.position += 1;
                }
                '(' => {
                    tokens.push(Token::LParen);
                    self.position += 1;
                }
                ')' => {
                    tokens.push(Token::RParen);
                    self.position += 1;
                }
                '=' => {
                    tokens.push(Token::Assign);
                    self.position += 1;
                }
                ',' => {
                    tokens.push(Token::Comma);
                    self.position += 1;
                }
                '$' => {
                    self.position += 1;
                    let var_name = self.read_word();
                    tokens.push(Token::Variable(var_name));
                }
                '"' | '\'' => {
                    let string_val = self.read_string(ch)?;
                    tokens.push(Token::Word(string_val));
                }
                _ => {
                    let word = self.read_word();
                    if word.is_empty() {
                        // Edge case fallback
                        self.position += 1;
                        continue;
                    }

                    // Handle multi-word keywords
                    if word == "and" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "then" {
                            self.read_word(); // Consume 'then'
                            tokens.push(Token::AndThen);
                        } else {
                            tokens.push(Token::Word(word));
                        }
                    } else if word == "or" {
                        self.skip_whitespace();
                        let next_word = self.peek_word();
                        if next_word == "else" {
                            self.read_word(); // Consume 'else'
                            tokens.push(Token::OrElse);
                        } else {
                            tokens.push(Token::Word(word));
                        }
                    } else {
                        // Check single keywords
                        match word.as_str() {
                            "to" => tokens.push(Token::RedirectTo),
                            "from" => tokens.push(Token::RedirectFrom),
                            "then" => tokens.push(Token::Then),
                            "while" => tokens.push(Token::WhileAsync), // Parser determines if it's async operator or while loop
                            "job" => tokens.push(Token::Job),
                            "if" => tokens.push(Token::If),
                            "elif" => tokens.push(Token::Elif),
                            "else" => tokens.push(Token::Else),
                            "for" => tokens.push(Token::For),
                            "foreach" => tokens.push(Token::Foreach),
                            "fn" => tokens.push(Token::Function),
                            _ => tokens.push(Token::Word(word)),
                        }
                    }
                }
            }
        }

        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.input.len() && self.input[self.position].is_whitespace() {
            self.position += 1;
        }
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while self.position < self.input.len() {
            let ch = self.input[self.position];
            if ch.is_whitespace() || "{}()=,:\"'$".contains(ch) {
                break;
            }
            word.push(ch);
            self.position += 1;
        }
        word
    }

    fn peek_word(&mut self) -> String {
        let original_pos = self.position;
        let word = self.read_word();
        self.position = original_pos;
        word
    }

    fn read_string(&mut self, quote: char) -> Result<String, crate::error::IshError> {
        self.position += 1; // skip opening quote
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
                self.position += 1; // skip closing quote
                return Ok(string_val);
            } else {
                string_val.push(ch);
            }
            self.position += 1;
        }

        Err(crate::error::IshError::ParseError("Unterminated string".to_string()))
    }
}
