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
            return Ok(AstNode::new(AstNodeKind::NamespaceDecl {
                name: String::new(),
                body: vec![],
            }, line, col));
        }

        Ok(AstNode::new(AstNodeKind::NamespaceDecl {
            name: String::new(),
            body: stmts,
        }, line, col))
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
        self.parse_statement()
    }

    fn parse_statement(&mut self) -> Result<AstNode, IshError> {
        if let Some(tok) = self.peek() {
            match tok.kind {
                TokenKind::If => self.parse_if_statement(),
                TokenKind::Switch => self.parse_switch_statement(),
                TokenKind::For | TokenKind::Foreach => self.parse_for_loop(),
                TokenKind::WhileAsync => self.parse_while_loop(),
                TokenKind::Public | TokenKind::Private | TokenKind::Protected | TokenKind::Internal | TokenKind::Static => {
                    if !self.in_declaration_context {
                        let (l, c) = self.get_location();
                        return Err(IshError::ParseError(format!("Declarations (func, class, struct, enum) are not allowed inside methods or blocks. Found '{:?}' at {}:{}", tok.kind, l, c)));
                    }
                    self.parse_declaration()
                }
                TokenKind::Class | TokenKind::Struct | TokenKind::Function | TokenKind::Enum => {
                    if !self.in_declaration_context {
                        let (l, c) = self.get_location();
                        return Err(IshError::ParseError(format!("Declarations (func, class, struct, enum) are not allowed inside methods or blocks. Found '{:?}' at {}:{}", tok.kind, l, c)));
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
                TokenKind::Word(_) => {
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
                        self.parse_assignment(false, None, None)
                    } else {
                        self.parse_expression()
                    }
                }
                _ => self.parse_expression(),
            }
        } else {
            Err(IshError::ParseError("Unexpected end of input".to_string()))
        }
    }

    fn get_precedence(tok: &TokenKind) -> u8 {
        match tok {
            TokenKind::Question => 1,
            TokenKind::OrElse => 2,
            TokenKind::AndThen => 3,
            TokenKind::Equals | TokenKind::NotEquals => 4,
            TokenKind::GreaterThan | TokenKind::LessThan | TokenKind::GreaterOrEq | TokenKind::LessOrEq => 5,
            TokenKind::Word(w) if w == "+" || w == "-" => 6,
            TokenKind::Word(w) if w == "*" || w == "/" || w == "%" => 7,
            _ => 0,
        }
    }

    fn parse_expression(&mut self) -> Result<AstNode, IshError> {
        self.parse_expression_pratt(0)
    }

    fn parse_expression_pratt(&mut self, min_prec: u8) -> Result<AstNode, IshError> {
        let mut left = self.parse_command()?;

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::LBracket) {
                if 90 < min_prec { break; }
                let l_tok = self.consume().ok_or_else(|| IshError::ParseError("Expected '['".to_string()))?;
                let index_expr = self.parse_expression()?;
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                    return Err(IshError::ParseError("Expected ']' after array index".to_string()));
                }
                self.consume(); // consume ]
                left = AstNode::new(AstNodeKind::IndexAccess {
                    object: Box::new(left),
                    index: Box::new(index_expr),
                }, l_tok.line, l_tok.column);
                continue;
            }

            let prec = Self::get_precedence(&tok.kind);
            if prec == 0 || prec < min_prec {
                break;
            }

            let op_tok = self.consume().ok_or_else(|| IshError::ParseError("Expected operator".to_string()))?;
            let op_str = match &op_tok.kind {
                TokenKind::Question => "?".to_string(),
                TokenKind::OrElse => "||".to_string(),
                TokenKind::AndThen => "&&".to_string(),
                TokenKind::Equals => "==".to_string(),
                TokenKind::NotEquals => "!=".to_string(),
                TokenKind::GreaterThan => ">".to_string(),
                TokenKind::LessThan => "<".to_string(),
                TokenKind::GreaterOrEq => ">=".to_string(),
                TokenKind::LessOrEq => "<=".to_string(),
                TokenKind::Word(w) => w.clone(),
                _ => return Err(IshError::ParseError("Expected operator".to_string())),
            };

            if op_str == "?" {
                let true_val = self.parse_expression_pratt(0)?;
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
                    return Err(IshError::ParseError("Expected ':' in ternary operator".into()));
                }
                self.consume(); // consume ':'
                let false_val = self.parse_expression_pratt(prec)?;
                left = AstNode::new(AstNodeKind::TernaryOp {
                    condition: Box::new(left),
                    true_value: Box::new(true_val),
                    false_value: Box::new(false_val),
                }, op_tok.line, op_tok.column);
            } else {
                let right = self.parse_expression_pratt(prec + 1)?;
                left = AstNode::new(AstNodeKind::BinaryOp {
                    left: Box::new(left),
                    operator: op_str,
                    right: Box::new(right),
                }, op_tok.line, op_tok.column);
            }
        }
        Ok(left)
    }

    fn parse_assignment(&mut self, is_declaration: bool, existing_var_name: Option<String>, type_specifier: Option<String>) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        let var_name = if let Some(name) = existing_var_name.clone() {
            name
        } else if let Some(tok) = self.consume() {
            match tok.kind {
                TokenKind::Word(v) => v,
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
            self.consume().ok_or_else(|| IshError::ParseError("Expected '='".to_string()))?; // Consume TokenKind::Assign
        }
        
        // If the value is just a StringLiteral, we don't need to parse it as a full expression.
        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::StringLiteral(_))) {
            if let Some(tok) = self.consume() {
                if let TokenKind::StringLiteral(s) = tok.kind {
                    return Ok(AstNode::new(AstNodeKind::Assignment { type_specifier, variable: var_name, index, value: Box::new(AstNode::new(AstNodeKind::StringLiteral(s), line, col)), is_declaration }, line, col));
                }
            }
        }

        let value = self.parse_statement()?; 
        Ok(AstNode::new(AstNodeKind::Assignment { type_specifier, variable: var_name, index, value: Box::new(value), is_declaration }, line, col))
    }

    fn parse_switch_statement(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Switch
        
        if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
            return Err(IshError::ParseError("Expected '(' after 'switch'".to_string()));
        }
        self.consume().ok_or_else(|| IshError::ParseError("Expected '('".to_string()))?;
        
        let expression = self.parse_expression()?;

        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
            self.consume();
        } else {
            return Err(IshError::ParseError("Expected ')' after switch expression".to_string()));
        }

        if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
            return Err(IshError::ParseError("Expected '{' after switch expression".to_string()));
        }
        self.consume().ok_or_else(|| IshError::ParseError("Expected '{'".to_string()))?;

        let mut cases = Vec::new();
        let mut default_case = None;

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            if matches!(tok.kind, TokenKind::Case) {
                self.consume();
                let case_expr = self.parse_expression()?;
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
                    return Err(IshError::ParseError("Expected ':' after case expression".to_string()));
                }
                self.consume().ok_or_else(|| IshError::ParseError("Expected ':'".to_string()))?;
                
                let mut body = Vec::new();
                while let Some(inner) = self.peek() {
                    if matches!(inner.kind, TokenKind::Case | TokenKind::Default | TokenKind::RBrace) {
                        break;
                    }
                    if matches!(inner.kind, TokenKind::Semicolon) {
                        self.consume();
                        continue;
                    }
                    body.push(self.parse_statement()?);
                }
                cases.push((case_expr, body));
            } else if matches!(tok.kind, TokenKind::Default) {
                self.consume();
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
                    return Err(IshError::ParseError("Expected ':' after default".to_string()));
                }
                self.consume().ok_or_else(|| IshError::ParseError("Expected ':'".to_string()))?;
                
                let mut body = Vec::new();
                while let Some(inner) = self.peek() {
                    if matches!(inner.kind, TokenKind::Case | TokenKind::Default | TokenKind::RBrace) {
                        break;
                    }
                    if matches!(inner.kind, TokenKind::Semicolon) {
                        self.consume();
                        continue;
                    }
                    body.push(self.parse_statement()?);
                }
                default_case = Some(body);
            } else {
                return Err(IshError::ParseError(format!("Expected 'case', 'default', or '}}' inside switch, found {:?}", tok.kind)));
            }
        }

        Ok(AstNode::new(AstNodeKind::Switch {
            expression: Box::new(expression),
            cases,
            default_case,
        }, line, col))
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
        
        let variable = match self.consume().ok_or_else(|| IshError::ParseError("Expected variable name after 'for'".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(v) => v,
                _ => return Err(IshError::ParseError("Expected variable name after 'for'".to_string())),
            },
        };

        if let Some(tok) = self.peek() {
            if let TokenKind::Word(w) = &tok.kind {
                if w == "in" {
                    self.consume();
                }
            }
        }
        
        let iterable = self.parse_expression()?;
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
                    TokenKind::Word(v) => {
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
        match self.consume() {
            Some(tok) if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' for namespace body".to_string())),
        }
        
        let old_context = self.in_declaration_context;
        self.in_declaration_context = true; // Namespaces can contain declarations
        
        let mut body = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            body.push(self.parse_statement()?);
        }
        
        self.in_declaration_context = old_context;
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
                TokenKind::Var | TokenKind::StringKeyword | TokenKind::IntKeyword | TokenKind::FloatKeyword | TokenKind::BoolKeyword | TokenKind::List | TokenKind::Word(_) => {
                    let kw_tok = self.consume().unwrap();
                    let mut type_name = match kw_tok.kind {
                        TokenKind::Var => "var".to_string(),
                        TokenKind::StringKeyword => "string".to_string(),
                        TokenKind::IntKeyword => "int".to_string(),
                        TokenKind::FloatKeyword => "float".to_string(),
                        TokenKind::BoolKeyword => "bool".to_string(),
                        TokenKind::List => "List".to_string(),
                        TokenKind::Word(w) => w,
                        _ => unreachable!(),
                    };

                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LessThan)) {
                        self.consume(); // <
                        type_name.push('<');
                        while let Some(tok) = self.peek() {
                            if matches!(tok.kind, TokenKind::GreaterThan) {
                                self.consume(); // >
                                type_name.push('>');
                                break;
                            } else {
                                match self.consume().unwrap().kind {
                                    TokenKind::Word(w) => type_name.push_str(&w),
                                    TokenKind::StringKeyword => type_name.push_str("string"),
                                    TokenKind::IntKeyword => type_name.push_str("int"),
                                    TokenKind::FloatKeyword => type_name.push_str("float"),
                                    TokenKind::BoolKeyword => type_name.push_str("bool"),
                                    TokenKind::Comma => type_name.push(','),
                                    _ => return Err(IshError::ParseError("Invalid generic type argument".to_string())),
                                }
                            }
                        }
                    }

                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                        self.consume(); // [
                        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                            self.consume(); // ]
                            type_name.push_str("[]");
                        } else {
                            return Err(IshError::ParseError(format!("Expected ']' after '{}['", type_name)));
                        }
                    }

                    let (line, col) = self.get_location();
                    let name = match self.consume() {
                        Some(tok) => match tok.kind {
                            TokenKind::Word(w) => w,
                            _ => return Err(IshError::ParseError("Expected field name".to_string())),
                        },
                        None => return Err(IshError::ParseError("Expected field name".to_string())),
                    };

                    let mut default_value = None;
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                        self.consume(); // =
                        default_value = Some(Box::new(self.parse_expression()?));
                        if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
                            self.consume();
                        }
                    } else if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
                        self.consume();
                    }

                    return Ok(AstNode::new(AstNodeKind::FieldDecl {
                        name,
                        type_specifier: Some(type_name),
                        access,
                        is_static,
                        default_value,
                    }, line, col));
                }
                _ => Err(IshError::ParseError("Expected class, struct, enum, or function declaration after modifiers".to_string())),
            }
        } else {
            Err(IshError::ParseError("Expected declaration after modifiers".to_string()))
        }
    }

    fn parse_class(&mut self, access: crate::core::ast::AccessSpecifier, is_static: bool) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Class
        let name = match self.consume().ok_or_else(|| IshError::ParseError("Expected class name".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected class name".to_string())),
            },
        };

        let mut base_class = None;
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Colon) {
                self.consume(); // Consume Colon
                match self.consume().ok_or_else(|| IshError::ParseError("Expected base class name after ':'".to_string()))? {
                    tok => match tok.kind {
                        TokenKind::Word(w) => base_class = Some(w),
                        _ => return Err(IshError::ParseError("Expected base class name after ':'".to_string())),
                    },
                }
            }
        }

        match self.consume().ok_or_else(|| IshError::ParseError("Expected '{' after class name".to_string()))? {
            tok if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' after class name".to_string())),
        }
        
        let mut methods = Vec::new();
        let mut fields = Vec::new();
        
        let mut constructor = None;
        let mut destructor = None;

        let old_context = self.in_declaration_context;
        self.in_declaration_context = true; // Classes contain declarations

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            let decl = self.parse_declaration()?;
            match decl.kind {
                AstNodeKind::Function { name: ref fn_name, .. } => {
                    if fn_name == &name {
                        if let AstNodeKind::Function { access, params, body, .. } = decl.kind {
                            constructor = Some((access, params, body));
                        } else { unreachable!() }
                    } else if fn_name == &format!("~{}", name) {
                        if let AstNodeKind::Function { body, .. } = decl.kind {
                            destructor = Some(body);
                        } else { unreachable!() }
                    } else {
                        methods.push(decl);
                    }
                }
                _ => fields.push(decl),
            }
        }
        self.in_declaration_context = old_context;
        Ok(AstNode::new(AstNodeKind::ClassDecl { name, base_class, access, is_static, methods, fields, constructor, destructor }, line, col))
    }

    fn parse_struct(&mut self, access: crate::core::ast::AccessSpecifier) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Struct
        let name = match self.consume().ok_or_else(|| IshError::ParseError("Expected struct name".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected struct name".to_string())),
            },
        };

        match self.consume().ok_or_else(|| IshError::ParseError("Expected '{' after struct name".to_string()))? {
            tok if matches!(tok.kind, TokenKind::LBrace) => {}
            _ => return Err(IshError::ParseError("Expected '{' after struct name".to_string())),
        }
        
        let mut fields = Vec::new();
        let mut constructor = None;
        let mut destructor = None;

        let old_context = self.in_declaration_context;
        self.in_declaration_context = true;

        while let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::RBrace) {
                self.consume();
                break;
            }
            if matches!(tok.kind, TokenKind::Semicolon) {
                self.consume();
                continue;
            }
            
            let decl = self.parse_declaration()?;
            match decl.kind {
                AstNodeKind::Function { name: ref fn_name, .. } => {
                    if fn_name == &name {
                        if let AstNodeKind::Function { access, params, body, .. } = decl.kind {
                            constructor = Some((access, params, body));
                        } else { unreachable!() }
                    } else if fn_name == &format!("~{}", name) {
                        if let AstNodeKind::Function { body, .. } = decl.kind {
                            destructor = Some(body);
                        } else { unreachable!() }
                    } else {
                        return Err(IshError::ParseError("Expected field declaration in struct".to_string()));
                    }
                }
                AstNodeKind::FieldDecl { .. } => fields.push(decl),
                _ => return Err(IshError::ParseError("Expected field declaration in struct".to_string())),
            }
        }
        
        self.in_declaration_context = old_context;
        Ok(AstNode::new(AstNodeKind::StructDecl { name, access, fields, constructor, destructor }, line, col))
    }

    fn parse_enum(&mut self, access: crate::core::ast::AccessSpecifier) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Enum
        let name = match self.consume().ok_or_else(|| IshError::ParseError("Expected enum name".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected enum name".to_string())),
            },
        };

        match self.consume().ok_or_else(|| IshError::ParseError("Expected '{' after enum name".to_string()))? {
            tok if matches!(tok.kind, TokenKind::LBrace) => {}
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
            
            match self.consume().ok_or_else(|| IshError::ParseError("Expected enum variant name".to_string()))? {
                tok => match tok.kind {
                    TokenKind::Word(w) => variants.push(w),
                    _ => return Err(IshError::ParseError("Expected enum variant name".to_string())),
                },
            }
        }

        Ok(AstNode::new(AstNodeKind::EnumDecl { name, access, variants }, line, col))
    }

    fn parse_with_import(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::With
        
        let path = match self.consume().ok_or_else(|| IshError::ParseError("Expected namespace path after 'with'".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected namespace path after 'with'".to_string())),
            },
        };

        // Consume optional semicolon
        if let Some(tok) = self.peek() {
            if tok.kind == TokenKind::Semicolon {
                self.consume();
            }
        }

        Ok(AstNode::new(AstNodeKind::WithImport { path }, line, col))
    }

    fn parse_parameters(&mut self) -> Result<Vec<crate::core::ast::Param>, IshError> {
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

                    let mut is_variadic = false;
                    if matches!(tok.kind, TokenKind::Params) {
                        self.consume(); // params
                        is_variadic = true;
                    }

                    let mut type_name = None;
                    let mut is_array = false;
                    if let Some(tok) = self.peek() {
                        if matches!(tok.kind, TokenKind::StringKeyword | TokenKind::IntKeyword | TokenKind::FloatKeyword | TokenKind::BoolKeyword | TokenKind::List | TokenKind::Word(_)) {
                            let kw_tok = self.consume().ok_or_else(|| IshError::ParseError("Expected type keyword".to_string()))?;
                            let mut tn = match kw_tok.kind {
                                TokenKind::StringKeyword => "string".to_string(),
                                TokenKind::IntKeyword => "int".to_string(),
                                TokenKind::FloatKeyword => "float".to_string(),
                                TokenKind::BoolKeyword => "bool".to_string(),
                                TokenKind::List => "List".to_string(),
                                TokenKind::Word(w) => w,
                                _ => return Err(IshError::ParseError("Expected type keyword".to_string())),
                            };
                            
                            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LessThan)) {
                                self.consume(); // <
                                tn.push('<');
                                while let Some(tok) = self.peek() {
                                    if matches!(tok.kind, TokenKind::GreaterThan) {
                                        self.consume(); // >
                                        tn.push('>');
                                        break;
                                    } else {
                                        match self.consume().ok_or_else(|| IshError::ParseError("Expected generic type argument".to_string()))?.kind {
                                            TokenKind::Word(w) => tn.push_str(&w),
                                            TokenKind::StringKeyword => tn.push_str("string"),
                                            TokenKind::IntKeyword => tn.push_str("int"),
                                            TokenKind::FloatKeyword => tn.push_str("float"),
                                            TokenKind::BoolKeyword => tn.push_str("bool"),
                                            TokenKind::Comma => tn.push(','),
                                            _ => return Err(IshError::ParseError("Invalid generic type argument".to_string())),
                                        }
                                    }
                                }
                            }

                            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                                self.consume(); // [
                                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                                    self.consume(); // ]
                                    tn.push_str("[]");
                                    is_array = true;
                                } else {
                                    return Err(IshError::ParseError(format!("Expected ']' after '{}['", tn)));
                                }
                            }
                            type_name = Some(tn);
                        } else {
                            return Err(IshError::ParseError("Method parameters must have a type specifier".to_string()));
                        }
                    } else {
                        return Err(IshError::ParseError("Method parameters must have a type specifier".to_string()));
                    }

                    // Get parameter name
                    let name = match self.consume().ok_or_else(|| IshError::ParseError("Expected parameter name".to_string()))? {
                        t => match t.kind {
                            TokenKind::Word(w) => w,
                            _ => return Err(IshError::ParseError("Expected parameter name".to_string())),
                        },
                    };

                    // Check for default value assignment
                    let mut default_value = None;
                    if !is_variadic && matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                        self.consume(); // Consume '='
                        let expr = self.parse_expression()?;
                        default_value = Some(Box::new(expr));
                    }

                    if is_variadic && matches!(self.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                        return Err(IshError::ParseError("Variadic parameters (params) cannot have default values".to_string()));
                    }

                    params.push(crate::core::ast::Param { name, type_specifier: type_name, is_array, is_variadic, default_value });
                }
            }
        }
        Ok(params)
    }

    fn parse_function(&mut self, access: crate::core::ast::AccessSpecifier, is_static: bool) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        self.consume(); // Consume TokenKind::Function
        let name = match self.consume().ok_or_else(|| IshError::ParseError("Expected function name".to_string()))? {
            tok => match tok.kind {
                TokenKind::Word(w) => w,
                _ => return Err(IshError::ParseError("Expected function name".to_string())),
            },
        };

        let params = self.parse_parameters()?;

        if name == "Main" {
            let mut valid = false;
            if params.len() == 1 {
                let p = &params[0];
                if p.name == "args" && p.type_specifier.as_deref() == Some("string[]") && p.is_variadic {
                    valid = true;
                }
            }
            if !valid {
                return Err(IshError::ParseError("Main function must have exactly one parameter: 'params string[] args'".to_string()));
            }
        }

        let body = self.parse_block()?;
        Ok(AstNode::new(AstNodeKind::Function { name, params, body, access, is_static }, line, col))
    }

    fn parse_interpolated_expression(&mut self, expr_str: &str) -> Result<AstNode, IshError> {
        let mut tokenizer = crate::core::tokenizer::Tokenizer::new(expr_str);
        let tokens = tokenizer.tokenize()?;
        
        if tokens.is_empty() {
            return Err(IshError::ParseError("Empty expression in interpolation".to_string()));
        }
        
        let mut temp_parser = Parser::new(tokens);
        temp_parser.parse_expression()
    }

    fn parse_block(&mut self) -> Result<Vec<AstNode>, IshError> {
        match self.consume().ok_or_else(|| IshError::ParseError("Expected '{' for block".to_string()))? {
            tok if matches!(tok.kind, TokenKind::LBrace) => {}
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

    fn parse_command(&mut self) -> Result<AstNode, IshError> {
        let (line, col) = self.get_location();
        
        if let Some(tok) = self.peek() {
            if matches!(tok.kind, TokenKind::Var | TokenKind::StringKeyword | TokenKind::IntKeyword | TokenKind::FloatKeyword | TokenKind::BoolKeyword) {
                let kw_tok = self.consume().unwrap();
                let mut type_name = match kw_tok.kind {
                    TokenKind::Var => "var",
                    TokenKind::StringKeyword => "string",
                    TokenKind::IntKeyword => "int",
                    TokenKind::FloatKeyword => "float",
                    TokenKind::BoolKeyword => "bool",
                    _ => unreachable!(),
                }.to_string();

                let mut is_array = false;
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
                    self.consume(); // consume [
                    if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                        self.consume(); // consume ]
                        type_name.push_str("[]");
                        is_array = true;
                    } else {
                        return Err(IshError::ParseError(format!("Expected ']' after '{}['", type_name)));
                    }
                }
                let assign_node = self.parse_assignment(true, None, Some(type_name))?;
                if is_array {
                    if let AstNodeKind::Assignment { value, .. } = &assign_node.kind {
                        if !matches!(&value.kind, AstNodeKind::Array(_)) {
                            return Err(IshError::ParseError("Array declaration must be initialized with an array [...]".to_string()));
                        }
                    }
                }
                return Ok(assign_node);
            }
            if matches!(tok.kind, TokenKind::New) {
                self.consume(); // Consume TokenKind::New
                let mut class_name = match self.consume() {
                    Some(tok) => match tok.kind {
                        TokenKind::Word(w) => w,
                        TokenKind::List => "List".to_string(),
                        _ => return Err(IshError::ParseError("Expected class name after new".to_string())),
                    },
                    None => return Err(IshError::ParseError("Expected class name after new".to_string())),
                };
                
                if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LessThan)) {
                    self.consume(); // <
                    class_name.push('<');
                    while let Some(tok) = self.peek() {
                        if matches!(tok.kind, TokenKind::GreaterThan) {
                            self.consume(); // >
                            class_name.push('>');
                            break;
                        } else {
                            match self.consume().unwrap().kind {
                                TokenKind::Word(w) => class_name.push_str(&w),
                                TokenKind::StringKeyword => class_name.push_str("string"),
                                TokenKind::IntKeyword => class_name.push_str("int"),
                                TokenKind::FloatKeyword => class_name.push_str("float"),
                                TokenKind::BoolKeyword => class_name.push_str("bool"),
                                TokenKind::Comma => class_name.push(','),
                                _ => return Err(IshError::ParseError("Invalid generic type argument".to_string())),
                            }
                        }
                    }
                }
                
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
                        args.push(self.parse_expression()?);
                    }
                }
                
                return Ok(AstNode::new(AstNodeKind::ObjectInstantiation { class_name, args }, line, col));
            }
            if matches!(tok.kind, TokenKind::LBracket) {
                self.consume();
                let mut items = Vec::new();
                while self.position < self.tokens.len() && !matches!(self.tokens[self.position].kind, TokenKind::RBracket) {
                    items.push(self.parse_expression()?);
                    if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                        self.consume();
                    }
                }
                if self.position < self.tokens.len() {
                    self.consume();
                }
                return Ok(AstNode::new(AstNodeKind::Array(items), line, col));
            }
            if matches!(tok.kind, TokenKind::LParen) {
                self.consume();
                let inner = self.parse_expression()?;
                if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
                    return Err(IshError::ParseError("Expected ')'".to_string()));
                }
                self.consume();
                return Ok(inner);
            }
        }

        if let Some(tok) = self.peek() {
            if let TokenKind::Word(w) = &tok.kind {
                if w == "Map" && self.position + 1 < self.tokens.len() && matches!(self.tokens[self.position + 1].kind, TokenKind::LParen) {
                    self.consume(); // Map
                    self.consume(); // (
                    
                    let mut items = Vec::new();
                    while self.position < self.tokens.len() && !matches!(self.tokens[self.position].kind, TokenKind::RParen) {
                        if !matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
                            return Err(IshError::ParseError("Expected '{' for map entry".to_string()));
                        }
                        self.consume(); // {
                        
                        let key_node = self.parse_expression()?;
                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Colon) {
                            self.consume(); // :
                        } else {
                            return Err(IshError::ParseError("Expected ':' after Map key".to_string()));
                        }
                        
                        let val_node = self.parse_expression()?;
                        
                        let key_str = match &key_node.kind {
                            AstNodeKind::Variable(var_name) => var_name.clone(),
                            AstNodeKind::StringLiteral(s) => s.clone(),
                            _ => "unknown_key".to_string(),
                        };
                        
                        items.push((key_str, val_node));

                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::RBrace) {
                            self.consume(); // }
                        } else {
                            return Err(IshError::ParseError("Expected '}' after map entry".to_string()));
                        }

                        if self.position < self.tokens.len() && matches!(self.tokens[self.position].kind, TokenKind::Comma) {
                            self.consume(); // ,
                        }
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
                TokenKind::InterpolatedStringLiteral(w) => {
                    let p = w.clone();
                    self.consume();
                    let mut nodes = Vec::new();
                    let mut current_str = String::new();
                    let mut in_expr = false;
                    let mut expr_start = 0;
                    let chars: Vec<char> = p.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        if chars[i] == '{' && !in_expr {
                            if !current_str.is_empty() {
                                nodes.push(AstNode::new(AstNodeKind::StringLiteral(current_str.clone()), line, col));
                                current_str.clear();
                            }
                            in_expr = true;
                            expr_start = i + 1;
                        } else if chars[i] == '}' && in_expr {
                            let expr_str: String = chars[expr_start..i].iter().collect();
                            match self.parse_interpolated_expression(&expr_str) {
                                Ok(expr_node) => nodes.push(expr_node),
                                Err(e) => return Err(e),
                            }
                            current_str.clear();
                            in_expr = false;
                        } else if !in_expr {
                            current_str.push(chars[i]);
                        }
                        i += 1;
                    }
                    if !current_str.is_empty() {
                        nodes.push(AstNode::new(AstNodeKind::StringLiteral(current_str.clone()), line, col));
                    }
                    return Ok(AstNode::new(AstNodeKind::InterpolatedString(nodes), line, col));
                }
                TokenKind::Word(w) => {
                    let p = w.clone();
                    self.consume();
                    if p.parse::<f64>().is_ok() || p == "true" || p == "false" || p == "null" {
                        is_string_literal = true;
                    } else if p.contains('.') || matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                        let (obj_name, member_name) = if p.contains('.') {
                            let parts: Vec<&str> = p.rsplitn(2, '.').collect();
                            (parts[1].to_string(), parts[0].to_string())
                        } else {
                            ("___implicit___".to_string(), p.clone())
                        };
                        
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
                                args.push(self.parse_expression()?);
                            }
                            return Ok(AstNode::new(AstNodeKind::MethodCall {
                                object: Box::new(AstNode::new(AstNodeKind::Variable(obj_name), line, col)),
                                method_name: member_name,
                                args
                            }, line, col));
                        } else {
                            return Ok(AstNode::new(AstNodeKind::PropertyAccess {
                                object: Box::new(AstNode::new(AstNodeKind::Variable(obj_name), line, col)),
                                property_name: member_name,
                            }, line, col));
                        }
                    }
                    p
                }
                TokenKind::Word(v) => {
                    let v = v.clone();
                    self.consume();
                    if v.contains('.') || matches!(self.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                        let (obj_name, member_name) = if v.contains('.') {
                            let parts: Vec<&str> = v.rsplitn(2, '.').collect();
                            (parts[1].to_string(), parts[0].to_string())
                        } else {
                            ("___implicit___".to_string(), v.clone())
                        };
                        
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
                                args.push(self.parse_expression()?);
                            }
                            return Ok(AstNode::new(AstNodeKind::MethodCall {
                                object: Box::new(AstNode::new(AstNodeKind::Variable(obj_name.to_string()), line, col)),
                                method_name: member_name,
                                args
                            }, line, col));
                        } else {
                            return Ok(AstNode::new(AstNodeKind::PropertyAccess {
                                object: Box::new(AstNode::new(AstNodeKind::Variable(obj_name.to_string()), line, col)),
                                property_name: member_name,
                            }, line, col));
                        }
                    }
                    
                    is_string_literal = true;
                    let mut result = v.clone();
                    
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
                                    TokenKind::Word(iv) => result.push_str(&iv.clone()),
                                    _ => {}
                                }
                                self.consume();
                            }
                        }
                    }
                    
                    result
                }

                _ => return Err(IshError::ParseError(format!("Expected command name, found {:?} at line {}, col {}", tok.kind, line, col))),
            }
        } else {
            return Err(IshError::ParseError("Expected command name".to_string()));
        };


        let mut args = Vec::new();

        while let Some(tok) = self.peek() {
            match &tok.kind {
                TokenKind::Word(word) => {
                    if ["*", "/", "+", "-", "==", "!=", "<", ">", "<=", ">=", "&&", "||", "?", ":"].contains(&word.as_str()) {
                        break;
                    }
                    args.push(word.clone());
                    self.consume();
                }
                TokenKind::StringLiteral(word) => {
                    args.push(word.clone());
                    self.consume();
                }
                TokenKind::Assign => {
                    args.push("=".to_string());
                    self.consume();
                }
                TokenKind::Word(var) => {
                    let mut var_str = var.clone();
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
                                    TokenKind::Word(iv) => var_str.push_str(&iv.clone()),
                                    _ => {}
                                }
                                self.consume();
                            }
                        }
                    }
                    args.push(var_str);
                }
                _ => break,
            }
        }

        if is_string_literal && args.is_empty() {
            return Ok(AstNode::new(AstNodeKind::StringLiteral(program), line, col));
        }

        if !args.is_empty() {
            return Err(IshError::ParseError(format!("Unexpected arguments after identifier '{}'. Did you forget parentheses for a method call?", program)));
        }

        Ok(AstNode::new(AstNodeKind::Variable(program), line, col))
    }
}