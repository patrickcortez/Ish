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
        let mut node = self.parse_pipeline()?;

        while let Some(tok) = self.peek() {
            match tok {
                Token::Then => {
                    self.consume();
                    let right = self.parse_pipeline()?;
                    node = AstNode::Sequential(Box::new(node), Box::new(right));
                }
                Token::AndThen => {
                    self.consume();
                    let right = self.parse_pipeline()?;
                    node = AstNode::AndThen(Box::new(node), Box::new(right));
                }
                Token::OrElse => {
                    self.consume();
                    let right = self.parse_pipeline()?;
                    node = AstNode::OrElse(Box::new(node), Box::new(right));
                }
                Token::WhileAsync => {
                    self.consume();
                    let right = self.parse_pipeline()?;
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
        // Extract program name
        let mut args = Vec::new();
        let mut redirect_to = None;
        let mut redirect_from = None;

        let program = if let Some(Token::Word(w)) = self.consume() {
            w
        } else {
            return Err(IshError::ParseError("Expected command name".to_string()));
        };

        while let Some(tok) = self.peek() {
            match tok {
                Token::Word(_) | Token::Variable(_) => {
                    // For now, treat Variable resolution as happening in executor.
                    // We just capture the raw name or structure here.
                    // To keep AST simple, we'll store '$var' as an arg.
                    match tok {
                        Token::Variable(var) => args.push(format!("${}", var)),
                        Token::Word(word) => args.push(word.clone()),
                        _ => {}
                    }
                    self.consume();
                }
                Token::RedirectTo => {
                    self.consume();
                    if let Some(Token::Word(w)) = self.consume() {
                        redirect_to = Some(w);
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
        })
    }
}
