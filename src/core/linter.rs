use crate::core::ast::AstNode;
use crate::error::IshError;

pub struct Linter {}

impl Linter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn lint(&self, ast: &AstNode) -> Result<(), IshError> {
        self.check_node(ast)
    }

    fn check_node(&self, node: &AstNode) -> Result<(), IshError> {
        match node {
            AstNode::Command { program, args: _, redirect_to, redirect_from } => {
                if program.is_empty() {
                    return Err(IshError::ParseError("Empty command detected".to_string()));
                }

                // Check invalid redirection combinations
                if redirect_to.is_some() && redirect_to == redirect_from {
                    return Err(IshError::ParseError(format!(
                        "Cannot redirect to and from the same file: {:?}",
                        redirect_to
                    )));
                }
            }
            AstNode::Pipeline(nodes) => {
                if nodes.len() < 2 {
                    return Err(IshError::ParseError("Pipeline requires at least two commands".to_string()));
                }
                for n in nodes {
                    self.check_node(n)?;
                }
            }
            AstNode::Sequential(left, right)
            | AstNode::AndThen(left, right)
            | AstNode::OrElse(left, right)
            | AstNode::Parallel(left, right) => {
                self.check_node(left)?;
                self.check_node(right)?;
            }
            AstNode::Background(inner) => {
                self.check_node(inner)?;
            }
            AstNode::If { condition, body, else_body } => {
                self.check_node(condition)?;
                for n in body {
                    self.check_node(n)?;
                }
                if let Some(eb) = else_body {
                    for n in eb {
                        self.check_node(n)?;
                    }
                }
            }
            AstNode::For { variable, iterable, body } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError("For loop missing variable name".to_string()));
                }
                self.check_node(iterable)?;
                for n in body {
                    self.check_node(n)?;
                }
            }
            AstNode::Assignment { variable, value } => {
                if variable.is_empty() {
                    return Err(IshError::ParseError("Assignment missing variable name".to_string()));
                }
                self.check_node(value)?;
            }
        }
        Ok(())
    }
}
