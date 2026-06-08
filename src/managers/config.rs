use anyhow::Result;
use std::path::PathBuf;

/// Represents the global configuration of Ish.
#[derive(Debug, Clone)]
pub struct Config {
    pub autocd: bool,
    pub suggestions: bool,
    pub editor: String,
    // Add color config later based on needs
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autocd: true,
            suggestions: true,
            editor: String::from("vim"), // Default editor
        }
    }
}

pub struct ConfigManager {
    config_path: PathBuf,
    pub current_config: Config,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let mut path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        path.push(".ishrc");
        
        let mut manager = Self {
            config_path: path,
            current_config: Config::default(),
        };
        
        manager.load()?;
        Ok(manager)
    }

    pub fn load(&mut self) -> Result<()> {
        if self.config_path.exists() {
            let content = std::fs::read_to_string(&self.config_path)?;
            for line in content.lines() {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    
                    match key {
                        "autocd" => self.current_config.autocd = value == "true",
                        "suggestions" => self.current_config.suggestions = value == "true",
                        "editor" => self.current_config.editor = value.to_string(),
                        _ => {}
                    }
                }
            }
        } else {
            self.save()?;
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let mut content = String::new();
        content.push_str(&format!("autocd={}\n", self.current_config.autocd));
        content.push_str(&format!("suggestions={}\n", self.current_config.suggestions));
        content.push_str(&format!("editor={}\n", self.current_config.editor));
        
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }
}
