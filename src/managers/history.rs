use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct HistoryManager {
    history: Vec<String>,
    current_index: usize,
    file_path: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Self {
        let home_dir = if cfg!(target_os = "windows") {
            std::env::var("USERPROFILE").unwrap_or_else(|_| String::from("."))
        } else {
            std::env::var("HOME").unwrap_or_else(|_| String::from("."))
        };
        let mut path = PathBuf::from(home_dir);
        path.push(".ish");
        if !path.exists() {
            let _ = std::fs::create_dir_all(&path);
        }
        path.push("history.txt");

        let mut manager = Self {
            history: Vec::new(),
            current_index: 0,
            file_path: path,
        };
        manager.load();
        manager
    }

    fn load(&mut self) {
        if let Ok(mut file) = OpenOptions::new().read(true).open(&self.file_path) {
            let mut content = String::new();
            if file.read_to_string(&mut content).is_ok() {
                self.history = content
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
        self.current_index = self.history.len();
    }

    pub fn save(&self) {
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.file_path)
        {
            for line in &self.history {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    pub fn add(&mut self, command: String) {
        let cmd = command.trim().to_string();
        if cmd.is_empty() {
            return;
        }

        // Avoid adding duplicates of the immediate previous command
        if self.history.last() != Some(&cmd) {
            self.history.push(cmd);
            self.save();
        }
        self.current_index = self.history.len();
    }

    pub fn get_previous(&mut self) -> Option<String> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(self.history[self.current_index].clone())
        } else {
            None
        }
    }

    pub fn get_next(&mut self) -> Option<String> {
        if self.current_index + 1 < self.history.len() {
            self.current_index += 1;
            Some(self.history[self.current_index].clone())
        } else {
            self.current_index = self.history.len();
            Some(String::new())
        }
    }

    pub fn get_all(&self) -> &[String] {
        &self.history
    }

    pub fn clear(&mut self) {
        self.history.clear();
        self.current_index = 0;
        self.save();
    }
}
