use crate::core::ast::{AstNode, AstNodeKind};

pub fn generate_rust_code(asts: &[AstNode]) -> String {
    let mut code = String::new();

    code.push_str("use ish::core::ast::IshValue;\n");
    code.push_str("use ish::core::executor::Executor;\n");
    code.push_str("use ish::error::IshError;\n");
    code.push_str("use std::collections::HashMap;\n\n");

    for ast in asts {
        code.push_str(&compile_node(ast, 0));
    }
    
    code.push_str("\nfn main() -> Result<(), IshError> {\n");
    code.push_str("    let args = std::env::args().collect::<Vec<_>>();\n");
    code.push_str("    let config = ish::core::config::IshConfig::default();\n");
    code.push_str("    let mut executor = Executor::new(args.clone(), config);\n");
    
    // The entry point must be Program::Main
    code.push_str("    // Initialize scope\n");
    code.push_str("    executor.variables.push(HashMap::new());\n");
    code.push_str("    Program_Main(args.into_iter().map(|s| IshValue::String(s)).collect(), &mut executor)?;\n");
    code.push_str("    Ok(())\n");
    code.push_str("}\n");

    code
}

fn compile_node(node: &AstNode, indent: usize) -> String {
    let mut code = String::new();
    let prefix = "    ".repeat(indent);

    match &node.kind {
        AstNodeKind::NamespaceDecl { name, body } => {
            if !name.is_empty() {
                // We use flat prefixes for now (e.g., App_Program_Main)
            }
            for child in body {
                code.push_str(&compile_node(child, indent));
            }
        }
        AstNodeKind::ClassDecl { name, methods, .. } => {
            // For now, emit methods with a flat prefix
            for method in methods {
                if let AstNodeKind::Function { name: mname, body, .. } = &method.kind {
                    code.push_str(&format!("{}#[allow(non_snake_case)]\n", prefix));
                    code.push_str(&format!("{}pub fn {}_{}(args: Vec<IshValue>, executor: &mut Executor) -> Result<Option<IshValue>, IshError> {{\n", prefix, name, mname));
                    code.push_str(&format!("{}    executor.variables.push(HashMap::new());\n", prefix));
                    for stmt in body {
                        code.push_str(&compile_node(stmt, indent + 1));
                    }
                    code.push_str(&format!("{}    executor.variables.pop();\n", prefix));
                    code.push_str(&format!("{}    Ok(None)\n", prefix));
                    code.push_str(&format!("{}}}\n", prefix));
                }
            }
        }
        AstNodeKind::Function { name, body, .. } => {
            code.push_str(&format!("{}#[allow(non_snake_case)]\n", prefix));
            code.push_str(&format!("{}pub fn {}(args: Vec<IshValue>, executor: &mut Executor) -> Result<Option<IshValue>, IshError> {{\n", prefix, name));
            code.push_str(&format!("{}    executor.variables.push(HashMap::new());\n", prefix));
            for stmt in body {
                code.push_str(&compile_node(stmt, indent + 1));
            }
            code.push_str(&format!("{}    executor.variables.pop();\n", prefix));
            code.push_str(&format!("{}    Ok(None)\n", prefix));
            code.push_str(&format!("{}}}\n", prefix));
        }
        AstNodeKind::MethodCall { object, method_name, args } => {
            if let AstNodeKind::Variable(v) = &object.kind {
                if v == "CommandLine" && method_name == "OutputLine" {
                    code.push_str(&format!("{}println!(\"{{}}\", ", prefix));
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 { code.push_str(", "); }
                        code.push_str(&format!("executor.value_to_string(&{})", compile_node(arg, 0).trim()));
                    }
                    code.push_str(");\n");
                }
            }
        }
        AstNodeKind::StringLiteral(s) => {
            code.push_str(&format!("IshValue::String(\"{}\".to_string())", s));
        }
        AstNodeKind::Variable(v) => {
            // Very naive evaluation
            code.push_str(&format!("IshValue::String(\"{}\".to_string())", v));
        }
        _ => {
            code.push_str(&format!("{}// Unimplemented node: {:?}\n", prefix, node.kind));
        }
    }

    code
}
