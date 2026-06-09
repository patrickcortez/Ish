use crate::core::ast::{AstNode, AstNodeKind};
use crate::core::tokenizer::{Token, TokenKind};
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

    fn get_location(&self) -> (usize, usize) {
        if let Some(tok) = self.peek() {
            (tok.line, tok.column)
        } else if self.tokens.len() > 0 {
            let last = &self.tokens[self.tokens.len() - 1];
            (last.line, last.column)
        } else {
            (1, 1)
        }
    }

    fn parse_logical(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        let mut node = self.parse_statement()?;

        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::Then => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::new(AstNodeKind::Sequential(Box::new(node), Box::new(right)), line, col);
                }
                TokenKind::AndThen => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::new(AstNodeKind::AndThen(Box::new(node), Box::new(right)), line, col);
                }
                TokenKind::OrElse => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::new(AstNodeKind::OrElse(Box::new(node), Box::new(right)), line, col);
                }
                TokenKind::WhileAsync => {
                    self.consume();
                    let right = self.parse_statement()?;
                    node = AstNode::new(AstNodeKind::Parallel(Box::new(node), Box::new(right)), line, col);
                }
                TokenKind::Job => {
                    self.consume();
                    node = AstNode::new(AstNodeKind::Background(Box::new(node)), line, col);
                }
                _ => break,
            }
        }

        Ok(node)
    }

    fn parse_statement(&mut self) -> Result<AstNode, IshError> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::If => self.parse_if_statement(),
                TokenKind::For => self.parse_for_loop(),
                TokenKind::WhileAsync => self.parse_while_loop(),
                TokenKind::Function => self.parse_function(),
                TokenKind::Return => {
                    let (line, col) = self.get_location();
                    self.consume();
                    let val = self.parse_pipeline()?;
                    Ok(AstNode::new(AstNodeKind::Return(Box::new(val)), line, col))
                }
                TokenKind::Break => {
                    let (line, col) = self.get_location();
                    self.consume();
                    Ok(AstNode::new(AstNodeKind::Break, line, col))
                }
                TokenKind::Continue => {
                    let (line, col) = self.get_location();
                    self.consume();
                    Ok(AstNode::new(AstNodeKind::Continue, line, col))
                }
                TokenKind::Variable(_) => {
                    if self.position + 1 < self.tokens.len() && matches!(self.tokens[self.position + 1].kind, TokenKind::Assign) {
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
        let (line, col) = self.get_location();
        let var_name = if let Some(tok) = self.consume() {
            if let TokenKind::Variable(v) = tok.kind {
                v
            } else {
                return Err(IshError::ParseError("Expected variable for assignment".to_string()));
            }
        } else {
            return Err(IshError::ParseError("Expected variable for assignment".to_string()));
        };
        self.consume(); // Consume TokenKind::Assign
        let value = self.parse_statement()?; 
        Ok(AstNode::new(AstNodeKind::Assignment { variable: var_name, value: Box::new(value) }, line, col))
    }

    fn parse_if_statement(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::If or TokenKind::Elif
        
        let condition = self.parse_pipeline()?;
        let body = self.parse_block()?;
        
        let mut else_body = None;
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Else) {
                self.consume();
                else_body = Some(self.parse_block()?);
            } else if matches!(tok.kind, TokenKind::Elif) {
                let elif_node = self.parse_if_statement()?;
                else_body = Some(vec![elif_node]);
            }
        }
        
        Ok(AstNode::new(AstNodeKind::If {
            condition: Box::new(condition),
            body,
            else_body,
        }, line, col))
    }

    fn parse_for_loop(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::For
        
        let variable = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Variable(v) => v,
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected variable name after 'for'".to_string())),
            },
            None => return Err(IshError::ParseError("Expected variable name after 'for'".to_string())),
        };

        if let Some(tok) = self.peek() {
            if let TokenKind::Word(w) = &tok.kind {
                if w == "in" {
                    self.consume();
                }
            }
        }
        
        let iterable = self.parse_pipeline()?;
        let body = self.parse_block()?;
        
        Ok(AstNode::new(AstNodeKind::For {
            variable,
            iterable: Box::new(iterable),
            body,
        }, line, col))
    }

    fn parse_while_loop(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::WhileAsync
        let condition = self.parse_pipeline()?;
        let body = self.parse_block()?;
        Ok(AstNode::new(AstNodeKind::While { condition: Box::new(condition), body }, line, col))
    }

    fn parse_function(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Function
        let name = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected function name".to_string())),
            },
            None => return Err(IshError::ParseError("Expected function name".to_string())),
        };

        let mut params = Vec::new();
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::LParen) {
                self.consume();
                while let Some(tok) = self.peek() {
                    if matches!(tok.kind, TokenKind::RParen) {
                        self.consume();
                        break;
                    }
                    if matches!(tok.kind, TokenKind::Comma) {
                        self.consume();
                        continue;
                    }
                    match self.consume() {
                        Some(tok) => match tok.kind {
                            TokenKind::Word(w) | TokenKind::Variable(w) => params.push(w),
                            _ => return Err(IshError::ParseError("Expected parameter name".to_string())),
                        },
                        None => return Err(IshError::ParseError("Expected parameter name".to_string())),
                    }
                }
            }
        }

        let body = self.parse_block()?;
        Ok(AstNode::new(AstNodeKind::Function { name, params, body }, line, col))
    }

    fn parse_block(&mut self) -> Result<Vec<AstNode>, IshError> {
        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' for block".to_string())),
        }
        
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            stmts.push(self.parse_logical()?);
        }
        
        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::RBrace) => {}
            _ => return Err(IshError::ParseError("Expected '}' at end of block".to_string())),
        }
        
        Ok(stmts)
    }

    fn parse_pipeline(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        let mut commands = vec![self.parse_command()?];

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Pipe) {
                self.consume(); // Consume ':'
                commands.push(self.parse_command()?);
            } else {
                break;
            }
        }

        if commands.len() == 1 {
            Ok(commands.pop().unwrap())
        } else {
            Ok(AstNode::new(AstNodeKind::Pipeline(commands), line, col))
        }
    }

    fn parse_command(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::LBracket) {
                self.consume();
                let mut items = Vec::new();
                while self.position < self.tokens.len() && !matches!(self.tokens[self.position].kind, TokenKind::RBracket) {
                    items.push(self.parse_pipeline()?);
                    if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                        self.consume();
                    }
                }
                if self.position < self.tokens.len() {
                    self.consume();
                }
                return Ok(AstNode::new(AstNodeKind::Array(items), line, col));
            }
        }

        if let Some(tok) = self.peek() {
            if let TokenKind::Word(w) = &tok.kind {
                if w == "Map" && self.position + 1 < self.tokens.len() && matches!(self.tokens[self.position + 1].kind, TokenKind::LParen) {
                    self.consume();
                    self.consume();
                    let mut items = Vec::new();
                    while self.position < self.tokens.len() && !matches!(self.tokens[self.position].kind, TokenKind::RParen) {
                        let key_node = self.parse_pipeline()?;
                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                            self.consume();
                        } else {
                            return Err(IshError::ParseError("Expected comma after Map key".to_string()));
                        }
                        let val_node = self.parse_pipeline()?;
                        
                        let key_str = match &key_node.kind {
                            AstNodeKind::Command { program, .. } => program.clone(),
                            _ => "unknown_key".to_string(),
                        };
                        
                        items.push((key_str, val_node));

                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                            self.consume();
                        }
                    }
                    if self.position < self.tokens.len() {
                        self.consume();
                    }
                    return Ok(AstNode::new(AstNodeKind::Map(items), line, col));
                }
            }
        }

        let program = if let Some(tok) = self.peek() {
            match &tok.kind {
                TokenKind::Word(w) => {
                    let p = w.clone();
                    self.consume();
                    p
                }
                TokenKind::Variable(v) => {
                    let p = format!("${}", v);
                    self.consume();
                    p
                }
                _ => return Err(IshError::ParseError(format!("Expected command name, found line {}", tok.line))),
            }
        } else {
            return Err(IshError::ParseError("Expected command name".to_string()));
        };

        if let Some(tok) = self.peek() {
            let op = match tok.kind {
                TokenKind::Equals => Some("=="),
                TokenKind::NotEquals => Some("!="),
                TokenKind::GreaterThan => Some(">"),
                TokenKind::LessThan => Some("<"),
                TokenKind::GreaterOrEq => Some(">="),
                TokenKind::LessOrEq => Some("<="),
                _ => None,
            };

            if let Some(op_str) = op {
                self.consume(); // consume operator
                let right_val = if let Some(tok) = self.peek() {
                    match &tok.kind {
                        TokenKind::Word(w) => {
                            let r = w.clone();
                            self.consume();
                            r
                        }
                        TokenKind::Variable(v) => {
                            let r = format!("${}", v);
                            self.consume();
                            r
                        }
                        _ => return Err(IshError::ParseError("Expected value after operator".to_string())),
                    }
                } else {
                    return Err(IshError::ParseError("Expected value after operator".to_string()));
                };

                let left = AstNode::new(AstNodeKind::Command {
                    program, args: vec![], redirect_to: None, redirect_from: None, append_to: None, read_doc: None, merge_err: false,
                }, line, col);
                let right = AstNode::new(AstNodeKind::Command {
                    program: right_val, args: vec![], redirect_to: None, redirect_from: None, append_to: None, read_doc: None, merge_err: false,
                }, line, col);

                return Ok(AstNode::new(AstNodeKind::Condition {
                    left: Box::new(left),
                    operator: op_str.to_string(),
                    right: Box::new(right),
                }, line, col));
            }
        }

        let mut args = Vec::new();
        let mut redirect_to = None;
        let mut redirect_from = None;
        let mut append_to = None;
        let mut read_doc = None;
        let mut merge_err = false;

        while let Some(tok) = self.peek() {
            match &tok.kind {
                TokenKind::Word(word) => {
                    args.push(word.clone());
                    self.consume();
                }
                TokenKind::Variable(var) => {
                    args.push(format!("${}", var));
                    self.consume();
                }
                TokenKind::AppendTo => {
                    self.consume();
                    if let Some(tok) = self.consume() {
                        if let TokenKind::Word(w) = tok.kind {
                            append_to = Some(w);
                        } else {
                            return Err(IshError::ParseError("Expected file after 'append to'".to_string()));
                        }
                    } else if let Some(tok) = self.peek() {
                        if matches!(tok.kind, TokenKind::DevNull) {
                            self.consume();
                            append_to = Some("DevNull".to_string());
                        } else {
                            return Err(IshError::ParseError("Expected file after 'append to'".to_string()));
                        }
                    } else {
                        return Err(IshError::ParseError("Expected file after 'append to'".to_string()));
                    }
                }
                TokenKind::ReadDoc => {
                    self.consume();
                    if let Some(tok) = self.consume() {
                        if let TokenKind::Word(w) = tok.kind {
                            read_doc = Some(w);
                        } else {
                            return Err(IshError::ParseError("Expected EOF word after 'read doc'".to_string()));
                        }
                    } else {
                        return Err(IshError::ParseError("Expected EOF word after 'read doc'".to_string()));
                    }
                }
                TokenKind::MergeErr => {
                    self.consume();
                    merge_err = true;
                }
                TokenKind::RedirectTo => {
                    self.consume();
                    if let Some(tok) = self.consume() {
                        if let TokenKind::Word(w) = tok.kind {
                            redirect_to = Some(w);
                        } else if matches!(tok.kind, TokenKind::DevNull) {
                            redirect_to = Some("DevNull".to_string());
                        } else {
                            return Err(IshError::ParseError("Expected file after 'to'".to_string()));
                        }
                    } else if let Some(tok) = self.peek() {
                        if matches!(tok.kind, TokenKind::DevNull) {
                            self.consume();
                            redirect_to = Some("DevNull".to_string());
                        } else {
                            return Err(IshError::ParseError("Expected file after 'to'".to_string()));
                        }
                    } else {
                        return Err(IshError::ParseError("Expected file after 'to'".to_string()));
                    }
                }
                TokenKind::RedirectFrom => {
                    self.consume();
                    if let Some(tok) = self.consume() {
                        if let TokenKind::Word(w) = tok.kind {
                            redirect_from = Some(w);
                        } else {
                            return Err(IshError::ParseError("Expected file after 'from'".to_string()));
                        }
                    } else {
                        return Err(IshError::ParseError("Expected file after 'from'".to_string()));
                    }
                }
                _ => break,
            }
        }

        Ok(AstNode::new(AstNodeKind::Command {
            program,
            args,
            redirect_to,
            redirect_from,
            append_to,
            read_doc,
            merge_err,
        }, line, col))
    }
}
