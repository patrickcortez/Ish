use ish::core::ast::IshValue;
use ish::core::executor::Executor;
use ish::error::IshError;
use std::collections::HashMap;

#[allow(non_snake_case)]
pub fn Program_Main(args: Vec<IshValue>, executor: &mut Executor) -> Result<Option<IshValue>, IshError> {
    executor.variables.push(HashMap::new());
    println!("{}", executor.value_to_string(&IshValue::String("Hello from AOT compiled Ish!".to_string())));
    executor.variables.pop();
    Ok(None)
}

fn main() -> Result<(), IshError> {
    let args = std::env::args().collect::<Vec<_>>();
    let config = ish::core::config::IshConfig::default();
    let mut executor = Executor::new(args.clone(), config);
    // Initialize scope
    executor.variables.push(HashMap::new());
    Program_Main(args.into_iter().map(|s| IshValue::String(s)).collect(), &mut executor)?;
    Ok(())
}
