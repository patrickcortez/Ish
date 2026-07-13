use std::fs;
use std::env;

fn main() {
    let content = fs::read_to_string("test/07_oop_test.ish").unwrap();
    let mut tokenizer = ish::core::tokenizer::Tokenizer::new(&content);
    let tokens = tokenizer.tokenize().unwrap();
    for t in tokens {
        println!("{:?} at line {}, col {}", t.kind, t.line, t.column);
    }
}
