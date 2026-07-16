use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;
use crate::error::IshError;

pub fn build_project(generated_rust_code: &str, target_dir: &Path, ish_core_path: &Path) -> Result<(), IshError> {
    let build_dir = target_dir.join(".ish_build");
    if !build_dir.exists() {
        fs::create_dir_all(&build_dir).map_err(|e| IshError::ExecutionError(format!("Failed to create build dir: {}", e)))?;
    }
    
    // Create src directory
    let src_dir = build_dir.join("src");
    if !src_dir.exists() {
        fs::create_dir_all(&src_dir).map_err(|e| IshError::ExecutionError(format!("Failed to create src dir: {}", e)))?;
    }
    
    // Write generated code to main.rs
    let main_rs_path = src_dir.join("main.rs");
    fs::write(&main_rs_path, generated_rust_code)
        .map_err(|e| IshError::ExecutionError(format!("Failed to write main.rs: {}", e)))?;
        
    // Write Cargo.toml
    let cargo_toml_path = build_dir.join("Cargo.toml");
    let ish_core_path_str = ish_core_path.to_string_lossy().replace("\\", "/");
    let cargo_toml_content = format!(r#"
[package]
name = "ish_compiled"
version = "0.1.0"
edition = "2024"

[dependencies]
ish = {{ path = "{}" }}
"#, ish_core_path_str);

    fs::write(&cargo_toml_path, cargo_toml_content)
        .map_err(|e| IshError::ExecutionError(format!("Failed to write Cargo.toml: {}", e)))?;
        
    // Invoke bundled or global cargo
    let cargo_bin = if Path::new("toolchain/bin/cargo.exe").exists() {
        "toolchain/bin/cargo.exe"
    } else {
        "cargo"
    };
    
    let status = Command::new(cargo_bin)
        .current_dir(&build_dir)
        .args(["build", "--release"])
        .status()
        .map_err(|e| IshError::ExecutionError(format!("Failed to execute cargo build: {}", e)))?;
        
    if !status.success() {
        return Err(IshError::ExecutionError("Cargo build failed".to_string()));
    }
    
    // Copy the executable out
    let exe_name = if cfg!(windows) { "ish_compiled.exe" } else { "ish_compiled" };
    let compiled_exe = build_dir.join("target").join("release").join(exe_name);
    let final_exe = target_dir.join(exe_name);
    
    fs::copy(&compiled_exe, &final_exe)
        .map_err(|e| IshError::ExecutionError(format!("Failed to copy executable: {}", e)))?;
        
    println!("Successfully compiled to {}", final_exe.display());
    
    Ok(())
}
