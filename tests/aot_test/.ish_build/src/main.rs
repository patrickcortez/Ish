use ish::core::ast::IshValue;
use ish::core::gobbler::{Gobbler, HeapObject};
use ish::core::stdlib::{StdlibProvider, IshCommandLine};

pub struct Program {}
impl Program {
    #[allow(non_snake_case)]
    pub fn Main(_args: Vec<IshValue>, gobbler: &mut Gobbler) -> Option<IshValue> {
        IshCommandLine.execute_method("OutputLine", &[IshValue::String("Hello from AOT compiled Ish!".to_string())], gobbler).unwrap();
        None
    }
}

fn main() {
    let mut gobbler = Gobbler::new();
    let args = vec![];
    Program::Main(args, &mut gobbler);
}
