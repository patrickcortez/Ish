use crate::core::ast::{AstNode, AstNodeKind};

pub fn generate_rust_code(ast: &AstNode) -> String {
    let mut code = String::new();

    // Standard imports
    code.push_str("use ish::core::ast::IshValue;\n");
    code.push_str("use ish::core::gobbler::{Gobbler, HeapObject};\n");
    code.push_str("use ish::core::stdlib::{StdlibProvider, IshCommandLine};\n\n");

    code.push_str(&compile_node(ast, 0));
    
    // Main function to start execution
    code.push_str("\nfn main() {\n");
    code.push_str("    let mut gobbler = Gobbler::new();\n");
    // Standard Ish entry point is usually Program::Main
    code.push_str("    let args = vec![];\n");
    code.push_str("    Program::Main(args, &mut gobbler);\n");
    code.push_str("}\n");

    code
}

fn compile_node(node: &AstNode, indent: usize) -> String {
    let mut code = String::new();
    let prefix = "    ".repeat(indent);

    match &node.kind {
        AstNodeKind::NamespaceDecl { name, body } => {
            if !name.is_empty() {
                code.push_str(&format!("{}pub mod {} {{\n", prefix, name));
                code.push_str(&format!("{}    use super::*;\n", prefix));
                for child in body {
                    code.push_str(&compile_node(child, indent + 1));
                }
                code.push_str(&format!("{}}}\n", prefix));
            } else {
                for child in body {
                    code.push_str(&compile_node(child, indent));
                }
            }
        }
        AstNodeKind::ClassDecl { name, methods, .. } => {
            code.push_str(&format!("{}pub struct {} {{}}\n", prefix, name));
            code.push_str(&format!("{}impl {} {{\n", prefix, name));
            for method in methods {
                code.push_str(&compile_node(method, indent + 1));
            }
            code.push_str(&format!("{}}}\n", prefix));
        }
        AstNodeKind::Function { name, body, .. } => {
            code.push_str(&format!("{}#[allow(non_snake_case)]\n", prefix));
            code.push_str(&format!("{}pub fn {}(_args: Vec<IshValue>, gobbler: &mut Gobbler) -> Option<IshValue> {{\n", prefix, name));
            for stmt in body {
                code.push_str(&compile_node(stmt, indent + 1));
            }
            code.push_str(&format!("{}    None\n", prefix));
            code.push_str(&format!("{}}}\n", prefix));
        }
        AstNodeKind::MethodCall { object, method_name, args } => {
            if let AstNodeKind::Variable(v) = &object.kind {
                if v == "CommandLine" {
                    code.push_str(&format!("{}IshCommandLine.execute_method(\"{}\", &[", prefix, method_name));
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 { code.push_str(", "); }
                        code.push_str(&compile_node(arg, 0).trim());
                    }
                    code.push_str("], gobbler).unwrap();\n");
                }
            }
        }
        AstNodeKind::StringLiteral(s) => {
            code.push_str(&format!("IshValue::String(\"{}\".to_string())", s));
        }
        AstNodeKind::Variable(v) => {
            code.push_str(v);
        }
        _ => {
            code.push_str(&format!("{}/* Unimplemented node */\n", prefix));
        }
    }

    code
}
