use crate::core::ast::AstNode;
use crate::core::tokenizer::Token;
use crate::error::IshError;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, position: 0 }
    }

    pub fn parse(&mut self) -> Result<AstNode, IshError> {
        self.parse_logical()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn consume(&mut self) -> Option<Token> {
        if self.position < self.tokens.len() {
            let t = self.tokens[self.position].clone();
            self.position += 1;
            Some(t)
        } else {
            None
        }
    }

    fn parse_logical(&mut self) -> Result<AstNode, IshError> {
        let mut node = self.parse_statement()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Then => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::Sequential(Box::new(node), Box::new(right));
                }
                Token::AndThen => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::AndThen(Box::new(node), Box::new(right));
                }
                Token::OrElse => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::OrElse(Box::new(node), Box::new(right));
                }
                Token::WhileAsync => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::Parallel(Box::new(node), Box::new(right));
                }
                Token::Job => {
                    self.consume();
                    node = AstNode::Background(Box::new(node));
                }
                _ => break,
            }
        }

        Ok(node)
    }

    fn parse_statement(&mut self) -> Result<AstNode, IshError> {
        if let Some(tok) = self.peek() {
            match tok {
                Token::If => self.parse_if_statement(),
                Token::For => self.parse_for_loop(),
                Token::WhileAsync => self.parse_while_loop(),
                Token::Function => self.parse_function(),
                Token::Return => {
                    self.consume();
                    let val = self.parse_pipeline()?;
                    Ok(AstNode::Return(Box::new(val)))
                }
                Token::Variable(_) => {
                    if self.position + 1 < self.tokens.len() && matches!(self.tokens[self.position + 1], Token::Assign) {
                        self.parse_assignment()
                    } else {
                        self.parse_pipeline()
                    }
                }
                _ => self.parse_pipeline(),
            }
        } else {
            Err(IshError::ParseError("Unexpected end of input".to_string()))
        }
    }

    fn parse_assignment(&mut self) -> Result<AstNode, IshError> {
        let var_name = if let Some(Token::Variable(v)) = self.consume() {
            v
        } else {
            return Err(IshError::ParseError("Expected variable for assignment".to_string()));
        };
        self.consume(); // Consume Token::Assign
        let value = self.parse_statement()?; 
        Ok(AstNode::Assignment { variable: var_name, value: Box::new(value) })
    }

    fn parse_if_statement(&mut self) -> Result<AstNode, IshError> {
        self.consume(); // Consume Token::If or Token::Elif
        
        let condition = self.parse_pipeline()?;
        let body = self.parse_block()?;
        
        let mut else_body = None;
        if let Some(Token::Else) = self.peek() {
            self.consume();
            else_body = Some(self.parse_block()?);
        } else if let Some(Token::Elif) = self.peek() {
            let elif_node = self.parse_if_statement()?;
            else_body = Some(vec![elif_node]);
        }
        
        Ok(AstNode::If {
            condition: Box::new(condition),
            body,
            else_body,
        })
    }

    fn parse_for_loop(&mut self) -> Result<AstNode, IshError> {
        self.consume(); // Consume Token::For
        
        let variable = match self.consume() {
            Some(Token::Variable(v)) => v,
            Some(Token::Word(w)) => w,
            _ => return Err(IshError::ParseError("Expected variable name after 'for'".to_string())),
        };

        if let Some(Token::Word(w)) = self.peek() {
            if w == "in" {
                self.consume();
            }
        }
        
        let iterable = self.parse_pipeline()?;
        let body = self.parse_block()?;
        
        Ok(AstNode::For {
            variable,
            iterable: Box::new(iterable),
            body,
        })
    }

    fn parse_while_loop(&mut self) -> Result<AstNode, IshError> {
        self.consume(); // Consume Token::WhileAsync
        let condition = self.parse_pipeline()?;
        let body = self.parse_block()?;
        Ok(AstNode::While { condition: Box::new(condition), body })
    }

    fn parse_function(&mut self) -> Result<AstNode, IshError> {
        self.consume(); // Consume Token::Function
        let name = match self.consume() {
            Some(Token::Word(w)) => w,
            _ => return Err(IshError::ParseError("Expected function name".to_string())),
        };

        let mut params = Vec::new();
        if let Some(Token::LParen) = self.peek() {
            self.consume();
            while let Some(tok) = self.peek() {
                if matches!(tok, Token::RParen) {
                    self.consume();
                    break;
                }
                if matches!(tok, Token::Comma) {
                    self.consume();
                    continue;
                }
                match self.consume() {
                    Some(Token::Word(w)) | Some(Token::Variable(w)) => params.push(w),
                    _ => return Err(IshError::ParseError("Expected parameter name".to_string())),
                }
            }
        }

        let body = self.parse_block()?;
        Ok(AstNode::Function { name, params, body })
    }

    fn parse_block(&mut self) -> Result<Vec<AstNode>, IshError> {
        match self.consume() {
            Some(Token::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' for block".to_string())),
        }
        
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok, Token::RBrace) {
                break;
            }
            if matches!(tok, Token::Semicolon) {
                self.consume();
                continue;
            }
            stmts.push(self.parse_logical()?);
        }
        
        match self.consume() {
            Some(Token::RBrace) => {}
            _ => return Err(IshError::ParseError("Expected '}' at end of block".to_string())),
        }
        
        Ok(stmts)
    }

    fn parse_pipeline(&mut self) -> Result<AstNode, IshError> {
        let mut commands = vec![self.parse_command()?];

        while let Some(Token::Pipe) = self.peek() {
            self.consume(); // Consume ':'
            commands.push(self.parse_command()?);
        }

        if commands.len() == 1 {
            Ok(commands.pop().unwrap())
        } else {
            Ok(AstNode::Pipeline(commands))
        }
    }

    fn parse_command(&mut self) -> Result<AstNode, IshError> {
        let program = if let Some(Token::Word(w)) = self.peek() {
            let p = w.clone();
            self.consume();
            p
        } else if let Some(Token::Variable(v)) = self.peek() {
            let p = format!("${}", v);
            self.consume();
            p
        } else {
            return Err(IshError::ParseError("Expected command name".to_string()));
        };

        if let Some(tok) = self.peek() {
            let op = match tok {
                Token::Equals => Some("=="),
                Token::NotEquals => Some("!="),
                Token::GreaterThan => Some(">"),
                Token::LessThan => Some("<"),
                Token::GreaterOrEq => Some(">="),
                Token::LessOrEq => Some("<="),
                _ => None,
            };

            if let Some(op_str) = op {
                self.consume(); // consume operator
                let right_val = if let Some(Token::Word(w)) = self.peek() {
                    let r = w.clone();
                    self.consume();
                    r
                } else if let Some(Token::Variable(v)) = self.peek() {
                    let r = format!("${}", v);
                    self.consume();
                    r
                } else {
                    return Err(IshError::ParseError("Expected value after operator".to_string()));
                };

                let left = AstNode::Command {
                    program, args: vec![], redirect_to: None, redirect_from: None, append_to: None, read_doc: None, merge_err: false,
                };
                let right = AstNode::Command {
                    program: right_val, args: vec![], redirect_to: None, redirect_from: None, append_to: None, read_doc: None, merge_err: false,
                };

                return Ok(AstNode::Condition {
                    left: Box::new(left),
                    operator: op_str.to_string(),
                    right: Box::new(right),
                });
            }
        }

        let mut args = Vec::new();
        let mut redirect_to = None;
        let mut redirect_from = None;
        let mut append_to = None;
        let mut read_doc = None;
        let mut merge_err = false;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Word(_) | Token::Variable(_) => {
                    match tok {
                        Token::Variable(var) => args.push(format!("${}", var)),
                        Token::Word(word) => args.push(word.clone()),
                        _ => {}
                    }
                    self.consume();
                }
                Token::AppendTo => {
                    self.consume();
                    if let Some(Token::Word(w)) = self.consume() {
                        append_to = Some(w);
                    } else if let Some(Token::DevNull) = self.peek() {
                        self.consume();
                        append_to = Some("DevNull".to_string());
                    } else {
                        return Err(IshError::ParseError("Expected file after 'append to'".to_string()));
                    }
                }
                Token::ReadDoc => {
                    self.consume();
                    if let Some(Token::Word(w)) = self.consume() {
                        read_doc = Some(w);
                    } else {
                        return Err(IshError::ParseError("Expected EOF word after 'read doc'".to_string()));
                    }
                }
                Token::MergeErr => {
                    self.consume();
                    merge_err = true;
                }
                Token::RedirectTo => {
                    self.consume();
                    if let Some(Token::Word(w)) = self.consume() {
                        redirect_to = Some(w);
                    } else if let Some(Token::DevNull) = self.peek() {
                        self.consume();
                        redirect_to = Some("DevNull".to_string());
                    } else {
                        return Err(IshError::ParseError("Expected file after 'to'".to_string()));
                    }
                }
                Token::RedirectFrom => {
                    self.consume();
                    if let Some(Token::Word(w)) = self.consume() {
                        redirect_from = Some(w);
                    } else {
                        return Err(IshError::ParseError("Expected file after 'from'".to_string()));
                    }
                }
                _ => break,
            }
        }

        Ok(AstNode::Command {
            program,
            args,
            redirect_to,
            redirect_from,
            append_to,
            read_doc,
            merge_err,
        })
    }
}
