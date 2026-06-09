use anyhow::Result;
use std::path::PathBuf;

/// Represents the global configuration of Ish.
#[derive(Debug, Clone)]
pub struct Config {
    pub autocd: bool,
    pub suggestions: bool,
    pub editor: String,
    pub input_color: String,
    pub output_color: String,
    pub banner_color: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autocd: true,
            suggestions: true,
            editor: String::from("vim"), // Default editor
            input_color: String::from("LightBlue"),
            output_color: String::from("White"),
            banner_color: String::from("LightBlue"),
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
                        "input_color" => self.current_config.input_color = value.to_string(),
                        "output_color" => self.current_config.output_color = value.to_string(),
                        "banner_color" => self.current_config.banner_color = value.to_string(),
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
        content.push_str(&format!("input_color={}\n", self.current_config.input_color));
        content.push_str(&format!("output_color={}\n", self.current_config.output_color));
        content.push_str(&format!("banner_color={}\n", self.current_config.banner_color));
        
        std::fs::write(&self.config_path, content)?;
        Ok(())
    }
}
