use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;

fn get_vscode_settings_path() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA").ok().map(|appdata| {
            let mut p = PathBuf::from(appdata);
            p.push("Code");
            p.push("User");
            p.push("settings.json");
            p
        })
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME").ok().map(|home| {
            let mut p = PathBuf::from(home);
            p.push("Library");
            p.push("Application Support");
            p.push("Code");
            p.push("User");
            p.push("settings.json");
            p
        })
    } else {
        std::env::var("HOME").ok().map(|home| {
            let mut p = PathBuf::from(home);
            p.push(".config");
            p.push("Code");
            p.push("User");
            p.push("settings.json");
            p
        })
    }
}

fn set_vscode_font(font_name: &str) {
    if let Some(settings_path) = get_vscode_settings_path() {
        if settings_path.exists() {
            println!("Found VS Code settings at {:?}", settings_path);
            let content = fs::read_to_string(&settings_path).unwrap_or_default();
            
            // Try parsing as JSON
            let mut v: serde_json::Value = match serde_json::from_str(&content) {
                Ok(val) => val,
                Err(_) => {
                    println!("Failed to parse VS Code settings.json (could contain comments or trailing commas).");
                    return;
                }
            };
            
            if let Some(obj) = v.as_object_mut() {
                obj.insert("terminal.integrated.fontFamily".to_string(), serde_json::Value::String(font_name.to_string()));
            }
            
            if let Ok(new_content) = serde_json::to_string_pretty(&v) {
                let _ = fs::write(settings_path, new_content);
                println!("Updated VS Code terminal font to {}", font_name);
            }
        } else {
            println!("VS Code settings.json not found.");
        }
    }
}

#[cfg(windows)]
fn install_fonts_windows(source_dir: &Path) {
    let localappdata = std::env::var("LOCALAPPDATA").expect("LOCALAPPDATA not set");
    let mut dest_dir = PathBuf::from(localappdata);
    dest_dir.push("Microsoft");
    dest_dir.push("Windows");
    dest_dir.push("Fonts");
    
    fs::create_dir_all(&dest_dir).unwrap();
    
    if let Ok(entries) = fs::read_dir(source_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                let file_name = path.file_name().unwrap();
                let mut dest_file = dest_dir.clone();
                dest_file.push(file_name);
                
                let _ = fs::copy(&path, &dest_file);
                println!("Copied {:?} to {:?}", file_name, dest_file);
                
                // Add to registry
                use winreg::enums::*;
                use winreg::RegKey;
                
                let hkcu = RegKey::predef(HKEY_CURRENT_USER);
                if let Ok((key, _)) = hkcu.create_subkey("Software\\Microsoft\\Windows NT\\CurrentVersion\\Fonts") {
                    let font_name = file_name.to_string_lossy().to_string(); 
                    let _ = key.set_value(font_name, &dest_file.to_string_lossy().to_string());
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn install_fonts_unix(source_dir: &Path) {
    let is_termux = std::env::var("PREFIX").unwrap_or_default().contains("com.termux");
    
    if is_termux {
        let home = std::env::var("HOME").unwrap();
        let mut dest_dir = PathBuf::from(home);
        dest_dir.push(".termux");
        fs::create_dir_all(&dest_dir).unwrap();
        
        let mut target_font = None;
        if let Ok(entries) = fs::read_dir(source_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    target_font = Some(path);
                    break;
                }
            }
        }
        
        if let Some(font_path) = target_font {
            let mut dest_file = dest_dir.clone();
            dest_file.push("font.ttf");
            let _ = fs::copy(font_path, dest_file);
            println!("Copied font to ~/.termux/font.ttf");
            let _ = Command::new("termux-reload-settings").output();
            println!("Reloaded Termux settings.");
        }
    } else if cfg!(target_os = "macos") {
        let home = std::env::var("HOME").unwrap();
        let mut dest_dir = PathBuf::from(home);
        dest_dir.push("Library");
        dest_dir.push("Fonts");
        fs::create_dir_all(&dest_dir).unwrap();
        
        if let Ok(entries) = fs::read_dir(source_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    let file_name = path.file_name().unwrap();
                    let mut dest_file = dest_dir.clone();
                    dest_file.push(file_name);
                    let _ = fs::copy(&path, &dest_file);
                    println!("Copied {:?} to {:?}", file_name, dest_file);
                }
            }
        }
    } else {
        let home = std::env::var("HOME").unwrap();
        let mut dest_dir = PathBuf::from(home);
        dest_dir.push(".local");
        dest_dir.push("share");
        dest_dir.push("fonts");
        fs::create_dir_all(&dest_dir).unwrap();
        
        if let Ok(entries) = fs::read_dir(source_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ttf") {
                    let file_name = path.file_name().unwrap();
                    let mut dest_file = dest_dir.clone();
                    dest_file.push(file_name);
                    let _ = fs::copy(&path, &dest_file);
                    println!("Copied {:?} to {:?}", file_name, dest_file);
                }
            }
        }
        
        let _ = Command::new("fc-cache").arg("-f").output();
        println!("Rebuilt font cache using fc-cache");
    }
}

fn main() {
    let current_dir = std::env::current_dir().unwrap();
    let mut assets_dir = current_dir.clone();
    assets_dir.push("Assets");
    assets_dir.push("JetBrainsMono");
    
    if !assets_dir.exists() {
        println!("Font directory {:?} does not exist. Please make sure Assets/JetBrainsMono is present.", assets_dir);
        return;
    }
    
    println!("Installing fonts from {:?}", assets_dir);
    
    #[cfg(windows)]
    install_fonts_windows(&assets_dir);
    
    #[cfg(not(windows))]
    install_fonts_unix(&assets_dir);
    
    let font_name = "JetBrainsMono NFM";
    println!("Configuring terminals to use font: {}", font_name);
    
    set_vscode_font(font_name);
    
    println!("Font installation and configuration complete.");
}
