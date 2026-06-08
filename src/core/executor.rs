use crate::core::ast::AstNode;
use crate::error::IshError;

pub struct Executor {}

impl Executor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn execute(&mut self, _ast: &AstNode) -> Result<(), IshError> {
        // To be implemented in Phase 4
        Ok(())
    }
}
