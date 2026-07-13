use crate::core::ast::{AstNode, AstNodeKind};
use crate::core::tokenizer::{Token, TokenKind};
use crate::error::IshError;

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
    in_declaration_context: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, position: 0, in_declaration_context: true }
    }

    pub fn parse(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        let mut stmts = Vec::new();

        while self.position < self.tokens.len() {
            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
                self.consume();
                continue;
            }

            // Top-level enforcement
            if let Some(tok) = self.peek() {
                match tok.kind {
                    TokenKind::With | TokenKind::Enum | TokenKind::Class | TokenKind::Struct |
                    TokenKind::Namespace | TokenKind::Public | TokenKind::Private | 
                    TokenKind::Protected | TokenKind::Internal | TokenKind::Static => {
                        stmts.push(self.parse_logical()?);
                    }
                    _ => {
                        return Err(IshError::ParseError(format!("Top-level constraint violation: Cannot have statements outside a class, struct, enum, or with-import. Found: {:?}", tok.kind)));
                    }
                }
            } else {
                break;
            }
        }

        if stmts.is_empty() {
            // Return an empty command node if nothing was parsed
            return Ok(AstNode::new(AstNodeKind::Command {
                program: String::new(),
                args: vec![],
                redirect_to: None,
                redirect_from: None,
                append_to: None,
                read_doc: None,
                merge_err: false,
            }, line, col));
        }

        let mut root = stmts.pop().unwrap();
        while let Some(prev) = stmts.pop() {
            root = AstNode::new(AstNodeKind::Sequential(Box::new(prev), Box::new(root)), line, col);
        }

        Ok(root)
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    pub fn consume(&mut self) -> Option<Token> {
        if self.position < self.tokens.len() {
            let tok = self.tokens[self.position].clone();
            self.position += 1;
            Some(tok)
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
                TokenKind::For | TokenKind::Foreach => self.parse_for_loop(),
                TokenKind::WhileAsync => self.parse_while_loop(),
                TokenKind::Public | TokenKind::Private | TokenKind::Protected | TokenKind::Internal | TokenKind::Static => {
                    if !self.in_declaration_context {
                        return Err(IshError::ParseError("Declarations (func, class, struct, enum) are not allowed inside methods or blocks.".to_string()));
                    }
                    self.parse_declaration()
                }
                TokenKind::Class | TokenKind::Struct | TokenKind::Function | TokenKind::Enum => {
                    if !self.in_declaration_context {
                        return Err(IshError::ParseError("Declarations (func, class, struct, enum) are not allowed inside methods or blocks.".to_string()));
                    }
                    self.parse_declaration()
                }
                TokenKind::With => self.parse_with_import(),
                TokenKind::Namespace => self.parse_namespace(),
                TokenKind::Return => {
                    let (line, col) = self.get_location();
                    self.consume(); // Consume TokenKind::Return
                    let value = if self.peek().is_some() 
                        && !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBrace))
                        && !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Semicolon))
                        && !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Then))
                    {
                        self.parse_expression()?
                    } else {
                        AstNode::new(AstNodeKind::StringLiteral("null".to_string()), line, col)
                    };
                    Ok(AstNode::new(AstNodeKind::Return(Box::new(value)), line, col))
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
                TokenKind::Try => self.parse_try_catch_statement(),
                TokenKind::Variable(_) | TokenKind::Word(_) => {
                    let mut is_assignment = false;
                    let mut lookahead = self.position + 1;
                    if lookahead < self.tokens.len() && matches!(self.tokens[lookahead].kind, TokenKind::LBracket) {
                        while lookahead < self.tokens.len() && !matches!(self.tokens[lookahead].kind, TokenKind::RBracket) {
                            lookahead += 1;
                        }
                        if lookahead < self.tokens.len() && matches!(self.tokens[lookahead].kind, TokenKind::RBracket) {
                            lookahead += 1;
                        }
                    }
                    if lookahead < self.tokens.len() && matches!(self.tokens[lookahead].kind, TokenKind::Assign) {
                        is_assignment = true;
                    }
                    
                    if is_assignment {
                        self.parse_assignment(false, None)
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

    fn parse_expression(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        if let Some(tok) = self.peek() {
            if let TokenKind::Subshell(inner) = &tok.kind {
                let s = inner.clone();
                self.consume();
                let mut tokenizer = crate::core::tokenizer::Tokenizer::new(&s);
                let sub_tokens = tokenizer.tokenize()?;
                let mut sub_parser = Parser::new(sub_tokens);
                let inner_node = sub_parser.parse_pipeline()?;
                return Ok(AstNode::new(AstNodeKind::Subshell(Box::new(inner_node)), line, col));
            }
            if let TokenKind::Word(w) = &tok.kind {
                let invalid_cmds = ["out", "show", "cd", "export", "alias", "unalias", "kill", "fg", "jobs", "declare", "let"];
                if invalid_cmds.contains(&w.as_str()) {
                    return Err(IshError::ParseError(format!("Invalid expression: cannot use command '{}' as a value. Use a subshell $(...) to capture command output.", w)));
                }
            }
            // An expression is at most a single command, condition, literal, or array/map. It cannot be a pipeline.
            self.parse_command()
        } else {
            Err(IshError::ParseError("Unexpected end of input".to_string()))
        }
    }

    fn parse_assignment(&mut self, is_declaration: bool, existing_var_name: Option<String>) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        let var_name = if let Some(name) = existing_var_name.clone() {
            name
        } else if let Some(tok) = self.consume() {
            match tok.kind {
                TokenKind::Variable(v) => v,
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected variable for assignment".to_string())),
            }
        } else {
            return Err(IshError::ParseError("Expected variable for assignment".to_string()));
        };
        
        let mut index = None;
        if existing_var_name.is_none() {
            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                self.consume(); // [
                index = Some(Box::new(self.parse_expression()?));
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                    return Err(IshError::ParseError("Expected ']' after array index".to_string()));
                }
                self.consume(); // ]
            }
            if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                return Err(IshError::ParseError("Expected '=' in assignment".to_string()));
            }
            self.consume(); // Consume TokenKind::Assign
        }
        
        // If the value is just a StringLiteral, we don't need to parse it as a full expression.
        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::StringLiteral(_))) {
            if let Some(TokenKind::StringLiteral(s)) = self.consume().map(|t| t.kind) {
                return Ok(AstNode::new(AstNodeKind::Assignment { variable: var_name, index, value: Box::new(AstNode::new(AstNodeKind::StringLiteral(s), line, col)), is_declaration }, line, col));
            }
        }

        let value = self.parse_statement()?; 
        Ok(AstNode::new(AstNodeKind::Assignment { variable: var_name, index, value: Box::new(value), is_declaration }, line, col))
    }

    fn parse_if_statement(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::If or TokenKind::Elif
        if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
            return Err(IshError::ParseError("Expected '(' after 'if'".to_string()));
        }
        self.consume();
        
        let condition = self.parse_logical()?;

        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
            self.consume();
        } else {
            return Err(IshError::ParseError("Expected ')' after if condition".to_string()));
        }
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
        if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
            return Err(IshError::ParseError("Expected '(' after 'while'".to_string()));
        }
        self.consume();

        let condition = self.parse_logical()?;

        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
            self.consume();
        } else {
            return Err(IshError::ParseError("Expected ')' after while condition".to_string()));
        }
        let body = self.parse_block()?;
        Ok(AstNode::new(AstNodeKind::While { condition: Box::new(condition), body }, line, col))
    }

    fn parse_try_catch_statement(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Try
        let try_body = self.parse_block()?;

        let mut error_var = "err".to_string();
        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Catch)) {
            self.consume(); // Consume TokenKind::Catch
            if let Some(tok) = self.peek() {
                match &tok.kind {
                    TokenKind::Variable(v) => {
                        error_var = v.clone();
                        self.consume();
                    }
                    TokenKind::Word(w) => {
                        if w != "{" {
                            error_var = w.clone();
                            self.consume();
                        }
                    }
                    _ => {}
                }
            }
        } else {
            return Err(IshError::ParseError("Expected 'catch' block after 'try'".to_string()));
        }

        let catch_body = self.parse_block()?;

        Ok(AstNode::new(AstNodeKind::TryCatch {
            try_body,
            error_var,
            catch_body,
        }, line, col))
    }

    fn parse_namespace(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Namespace
        let name = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected namespace name".to_string())),
            },
            None => return Err(IshError::ParseError("Expected namespace name".to_string())),
        };
        let body = self.parse_block()?;
        Ok(AstNode::new(AstNodeKind::NamespaceDecl { name, body }, line, col))
    }

    fn parse_declaration(&mut self) -> Result<AstNode, IshError> {
        use crate::core::ast::AccessSpecifier;
        let mut access = AccessSpecifier::Internal; // default access
        let mut is_static = false;
        
        while let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::Public => { access = AccessSpecifier::Public; self.consume(); }
                TokenKind::Private => { access = AccessSpecifier::Private; self.consume(); }
                TokenKind::Protected => { access = AccessSpecifier::Protected; self.consume(); }
                TokenKind::Internal => { access = AccessSpecifier::Internal; self.consume(); }
                TokenKind::Static => { is_static = true; self.consume(); }
                _ => break,
            }
        }
        
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::Class => self.parse_class(access, is_static),
                TokenKind::Struct => self.parse_struct(access),
                TokenKind::Function => self.parse_function(access, is_static),
                TokenKind::Enum => self.parse_enum(access),
                _ => Err(IshError::ParseError("Expected class, struct, enum, or function declaration after modifiers".to_string())),
            }
        } else {
            Err(IshError::ParseError("Expected declaration after modifiers".to_string()))
        }
    }

    fn parse_class(&mut self, access: crate::core::ast::AccessSpecifier, is_static: bool) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Class
        let name = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected class name".to_string())),
            },
            None => return Err(IshError::ParseError("Expected class name".to_string())),
        };

        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' after class name".to_string())),
        }
        
        let mut methods = Vec::new();
        let mut fields = Vec::new();
        
        let mut constructor = None;
        let mut destructor = None;

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            if matches!(tok.kind, TokenKind::Constructor) {
                self.consume(); // consume 'constructor'
                // expect ()
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                    self.consume();
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
                        self.consume();
                    }
                }
                constructor = Some(self.parse_block()?);
                continue;
            }
            if matches!(tok.kind, TokenKind::Destructor) {
                self.consume(); // consume 'destructor'
                // expect ()
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                    self.consume();
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
                        self.consume();
                    }
                }
                destructor = Some(self.parse_block()?);
                continue;
            }
            // we expect methods or fields inside class
            let decl = self.parse_declaration()?;
            match decl.kind {
                AstNodeKind::Function { .. } => methods.push(decl),
                _ => fields.push(decl),
            }
        }

        Ok(AstNode::new(AstNodeKind::ClassDecl { name, access, is_static, methods, fields, constructor, destructor }, line, col))
    }

    fn parse_struct(&mut self, access: crate::core::ast::AccessSpecifier) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Struct
        let name = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected struct name".to_string())),
            },
            None => return Err(IshError::ParseError("Expected struct name".to_string())),
        };

        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' after struct name".to_string())),
        }
        
        let mut fields = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            // in a struct, maybe fields are declared with let
            if matches!(tok.kind, TokenKind::Let) {
                self.consume();
                fields.push(self.parse_assignment(true, None)?);
            } else if let TokenKind::Word(w) = &tok.kind {
                let var_name = w.clone();
                self.consume();
                let is_decl = true;
                let val = if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                    self.consume();
                    self.parse_expression()?
                } else {
                    AstNode::new(AstNodeKind::StringLiteral("".to_string()), line, col)
                };
                fields.push(AstNode::new(AstNodeKind::Assignment { variable: var_name, index: None, value: Box::new(val), is_declaration: is_decl }, line, col));
            } else {
                return Err(IshError::ParseError("Expected field declaration in struct".to_string()));
            }
        }
        
        Ok(AstNode::new(AstNodeKind::StructDecl { name, access, fields }, line, col))
    }

    fn parse_enum(&mut self, access: crate::core::ast::AccessSpecifier) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Enum
        let name = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected enum name".to_string())),
            },
            None => return Err(IshError::ParseError("Expected enum name".to_string())),
        };

        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' after enum name".to_string())),
        }
        
        let mut variants = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Comma) || matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            
            match self.consume() {
                Some(tok) => match tok.kind {
                    TokenKind::Word(w) | TokenKind::Variable(w) => variants.push(w),
                    _ => return Err(IshError::ParseError("Expected enum variant name".to_string())),
                },
                None => return Err(IshError::ParseError("Expected enum variant name".to_string())),
            }
        }

        Ok(AstNode::new(AstNodeKind::EnumDecl { name, access, variants }, line, col))
    }

    fn parse_with_import(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::With
        
        let path = match self.consume() {
            Some(tok) => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected namespace path after 'with'".to_string())),
            },
            None => return Err(IshError::ParseError("Expected namespace path after 'with'".to_string())),
        };

        // Consume optional semicolon
        if let Some(tok) = self.peek() {
            if tok.kind == TokenKind::Semicolon {
                self.consume();
            }
        }

        Ok(AstNode::new(AstNodeKind::WithImport { path }, line, col))
    }

    fn parse_function(&mut self, access: crate::core::ast::AccessSpecifier, is_static: bool) -> Result<AstNode, IshError> {
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
        let mut has_params = false;
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
                    if matches!(tok.kind, TokenKind::Params) {
                        self.consume(); // params
                        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Let)) {
                            self.consume(); // let
                            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                                self.consume(); // [
                                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                                    self.consume(); // ]
                                }
                            }
                        }
                        match self.consume() {
                            Some(t) => match t.kind {
                                TokenKind::Word(w) | TokenKind::Variable(w) => {
                                    params.push(w);
                                    has_params = true;
                                }
                                _ => return Err(IshError::ParseError("Expected param array name".to_string())),
                            },
                            None => return Err(IshError::ParseError("Expected param array name".to_string())),
                        }
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
        Ok(AstNode::new(AstNodeKind::Function { name, params, has_params, body, access, is_static }, line, col))
    }

    fn parse_block(&mut self) -> Result<Vec<AstNode>, IshError> {
        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' for block".to_string())),
        }
        
        let old_context = self.in_declaration_context;
        self.in_declaration_context = false;
        
        let mut stmts = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            let stmt_res = self.parse_logical();
            if stmt_res.is_err() {
                self.in_declaration_context = old_context;
                return stmt_res.map(|x| vec![x]); // bubble up error
            }
            stmts.push(stmt_res.unwrap());
        }
        
        self.in_declaration_context = old_context;
        
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
            if matches!(tok.kind, TokenKind::Declare) || matches!(tok.kind, TokenKind::Let) {
                self.consume(); // consume Let or Declare
                let mut is_array = false;
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                    self.consume(); // consume [
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                        self.consume(); // consume ]
                        is_array = true;
                    } else {
                        return Err(IshError::ParseError("Expected ']' after 'let['".to_string()));
                    }
                }
                let assign_node = self.parse_assignment(true, None)?;
                if is_array {
                    if let AstNodeKind::Assignment { value, .. } = &assign_node.kind {
                        if !matches!(&value.kind, AstNodeKind::Array(_)) {
                            return Err(IshError::ParseError("Array declaration 'let[]' must be initialized with an array [...]".to_string()));
                        }
                    }
                }
                return Ok(assign_node);
            }
            if matches!(tok.kind, TokenKind::New) {
                self.consume(); // Consume TokenKind::New
                let class_name = match self.consume() {
                    Some(tok) => match tok.kind {
                        TokenKind::Word(w) => w,
                        TokenKind::List => "List".to_string(),
                        _ => return Err(IshError::ParseError("Expected class name after new".to_string())),
                    },
                    None => return Err(IshError::ParseError("Expected class name after new".to_string())),
                };
                
                let mut args = Vec::new();
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
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
                        args.push(self.parse_pipeline()?);
                    }
                }
                
                return Ok(AstNode::new(AstNodeKind::ObjectInstantiation { class_name, args }, line, col));
            }
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
                    self.consume(); // Map
                    self.consume(); // (
                    
                    if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
                        return Err(IshError::ParseError("Expected '{' after 'Map('".to_string()));
                    }
                    self.consume(); // {
                    
                    let mut items = Vec::new();
                    while self.position < self.tokens.len() && !matches!(self.tokens[self.position].kind, TokenKind::RBrace) {
                        if matches!(self.tokens[self.position].kind, TokenKind::Semicolon) {
                            self.consume();
                            continue;
                        }
                        let key_node = self.parse_pipeline()?;
                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Colon) {
                            self.consume(); // :
                        } else {
                            return Err(IshError::ParseError("Expected ':' after Map key".to_string()));
                        }
                        
                        let val_node = self.parse_pipeline()?;
                        
                        let key_str = match &key_node.kind {
                            AstNodeKind::Command { program, .. } => program.clone(),
                            AstNodeKind::StringLiteral(s) => s.clone(),
                            _ => "unknown_key".to_string(),
                        };
                        
                        items.push((key_str, val_node));

                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                            self.consume(); // ,
                        }
                    }
                    if self.position < self.tokens.len() {
                        self.consume(); // }
                    }
                    if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
                        return Err(IshError::ParseError("Expected ')' after 'Map({...}'".to_string()));
                    }
                    self.consume(); // )
                    return Ok(AstNode::new(AstNodeKind::Map(items), line, col));
                }
            }
        }

        let mut is_string_literal = false;
        let program = if let Some(tok) = self.peek() {
            match &tok.kind {
                TokenKind::StringLiteral(w) => {
                    let p = w.clone();
                    self.consume();
                    is_string_literal = true;
                    p
                }
                TokenKind::Word(w) => {
                    let p = w.clone();
                    self.consume();
                    if p.parse::<f64>().is_ok() || p == "true" || p == "false" || p == "null" {
                        is_string_literal = true;
                    } else if p.contains('.') {
                        let parts: Vec<&str> = p.splitn(2, '.').collect();
                        let obj_name = parts[0].to_string();
                        let member_name = parts[1].to_string();
                        
                        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                            self.consume();
                            let mut args = Vec::new();
                            while let Some(tok) = self.peek() {
                                if matches!(tok.kind, TokenKind::RParen) {
                                    self.consume();
                                    break;
                                }
                                if matches!(tok.kind, TokenKind::Comma) {
                                    self.consume();
                                    continue;
                                }
                                args.push(self.parse_pipeline()?);
                            }
                            return Ok(AstNode::new(AstNodeKind::MethodCall {
                                object: Box::new(AstNode::new(AstNodeKind::StringLiteral(obj_name), line, col)),
                                method_name: member_name,
                                args
                            }, line, col));
                        } else {
                            return Ok(AstNode::new(AstNodeKind::PropertyAccess {
                                object: Box::new(AstNode::new(AstNodeKind::StringLiteral(obj_name), line, col)),
                                property_name: member_name,
                            }, line, col));
                        }
                    }
                    p
                }
                TokenKind::Variable(v) => {
                    let v = v.clone();
                    self.consume();
                    if v.contains('.') {
                        let parts: Vec<&str> = v.splitn(2, '.').collect();
                        let obj_name = format!("${}", parts[0]);
                        let member_name = parts[1].to_string();
                        
                        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                            self.consume();
                            let mut args = Vec::new();
                            while let Some(tok) = self.peek() {
                                if matches!(tok.kind, TokenKind::RParen) {
                                    self.consume();
                                    break;
                                }
                                if matches!(tok.kind, TokenKind::Comma) {
                                    self.consume();
                                    continue;
                                }
                                args.push(self.parse_pipeline()?);
                            }
                            return Ok(AstNode::new(AstNodeKind::MethodCall {
                                object: Box::new(AstNode::new(AstNodeKind::StringLiteral(obj_name), line, col)),
                                method_name: member_name,
                                args
                            }, line, col));
                        } else {
                            return Ok(AstNode::new(AstNodeKind::PropertyAccess {
                                object: Box::new(AstNode::new(AstNodeKind::StringLiteral(obj_name), line, col)),
                                property_name: member_name,
                            }, line, col));
                        }
                    }
                    
                    is_string_literal = true;
                    let mut result = format!("${}", v);
                    
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                        self.consume(); // [
                        result.push('[');
                        while let Some(tok) = self.peek() {
                            if matches!(tok.kind, TokenKind::RBracket) {
                                self.consume();
                                result.push(']');
                                break;
                            } else {
                                match &tok.kind {
                                    TokenKind::Word(w) => result.push_str(w),
                                    TokenKind::StringLiteral(w) => result.push_str(&format!("\"{}\"", w)),
                                    TokenKind::Variable(iv) => result.push_str(&format!("${}", iv)),
                                    _ => {}
                                }
                                self.consume();
                            }
                        }
                    }
                    
                    result
                }
                TokenKind::Subshell(w) => {
                    let p = w.clone();
                    self.consume();
                    
                    let mut inner_tokenizer = crate::core::tokenizer::Tokenizer::new(&p);
                    let ast_node = if let Ok(tokens) = inner_tokenizer.tokenize() {
                        let mut inner_parser = crate::core::parser::Parser::new(tokens);
                        if let Ok(ast) = inner_parser.parse() {
                            ast
                        } else {
                            AstNode::new(AstNodeKind::StringLiteral(p), line, col)
                        }
                    } else {
                        AstNode::new(AstNodeKind::StringLiteral(p), line, col)
                    };
                    return Ok(AstNode::new(AstNodeKind::Subshell(Box::new(ast_node)), line, col));
                }
                _ => return Err(IshError::ParseError(format!("Expected command name, found {:?} at line {}, col {}", tok.kind, line, col))),
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
                        TokenKind::Word(w) | TokenKind::StringLiteral(w) => {
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

                let left = AstNode::new(AstNodeKind::StringLiteral(program), line, col);
                let right = AstNode::new(AstNodeKind::StringLiteral(right_val), line, col);

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
                TokenKind::Word(word) | TokenKind::StringLiteral(word) => {
                    args.push(word.clone());
                    self.consume();
                }
                TokenKind::Assign => {
                    args.push("=".to_string());
                    self.consume();
                }
                TokenKind::Variable(var) => {
                    let mut var_str = format!("${}", var);
                    self.consume();
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                        self.consume(); // [
                        var_str.push('[');
                        while let Some(tok) = self.peek() {
                            if matches!(tok.kind, TokenKind::RBracket) {
                                self.consume();
                                var_str.push(']');
                                break;
                            } else {
                                match &tok.kind {
                                    TokenKind::Word(w) => var_str.push_str(w),
                                    TokenKind::StringLiteral(w) => var_str.push_str(&format!("\"{}\"", w)),
                                    TokenKind::Variable(iv) => var_str.push_str(&format!("${}", iv)),
                                    _ => {}
                                }
                                self.consume();
                            }
                        }
                    }
                    args.push(var_str);
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

        if is_string_literal && args.is_empty() && redirect_to.is_none() && redirect_from.is_none() && append_to.is_none() && read_doc.is_none() {
            return Ok(AstNode::new(AstNodeKind::StringLiteral(program), line, col));
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
