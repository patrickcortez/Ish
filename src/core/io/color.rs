use super::IshIOError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    RGB(u8, u8, u8),
}

impl Color {
    pub fn from_hex(hex: &str) -> Result<Color, IshIOError> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return Err(IshIOError::InvalidHexColor(hex.to_string()));
        }
        
        let r = u8::from_str_radix(&hex[0..2], 16)
            .map_err(|_| IshIOError::InvalidHexColor(hex.to_string()))?;
        let g = u8::from_str_radix(&hex[2..4], 16)
            .map_err(|_| IshIOError::InvalidHexColor(hex.to_string()))?;
        let b = u8::from_str_radix(&hex[4..6], 16)
            .map_err(|_| IshIOError::InvalidHexColor(hex.to_string()))?;
        
        Ok(Color::RGB(r, g, b))
    }
    
    fn to_ansi_code(&self, is_background: bool) -> String {
        match self {
            Color::Black => if is_background { "\x1b[40m" } else { "\x1b[30m" }.to_string(),
            Color::Red => if is_background { "\x1b[41m" } else { "\x1b[31m" }.to_string(),
            Color::Green => if is_background { "\x1b[42m" } else { "\x1b[32m" }.to_string(),
            Color::Yellow => if is_background { "\x1b[43m" } else { "\x1b[33m" }.to_string(),
            Color::Blue => if is_background { "\x1b[44m" } else { "\x1b[34m" }.to_string(),
            Color::Magenta => if is_background { "\x1b[45m" } else { "\x1b[35m" }.to_string(),
            Color::Cyan => if is_background { "\x1b[46m" } else { "\x1b[36m" }.to_string(),
            Color::White => if is_background { "\x1b[47m" } else { "\x1b[37m" }.to_string(),
            Color::RGB(r, g, b) => {
                let prefix = if is_background { 48 } else { 38 };
                format!("\x1b[{};2;{};{};{}m", prefix, r, g, b)
            }
        }
    }
}

pub struct ColorManager {
    foreground: Option<Color>,
    background: Option<Color>,
}

impl ColorManager {
    pub fn new() -> Self {
        Self {
            foreground: None,
            background: None,
        }
    }
    
    pub fn set_foreground(&mut self, color: Color) -> Result<(), IshIOError> {
        self.foreground = Some(color);
        let ansi_code = color.to_ansi_code(false);
        print!("{}", ansi_code);
        Ok(())
    }
    
    pub fn set_background(&mut self, color: Color) -> Result<(), IshIOError> {
        self.background = Some(color);
        let ansi_code = color.to_ansi_code(true);
        print!("{}", ansi_code);
        Ok(())
    }
    
    pub fn reset(&mut self) -> Result<(), IshIOError> {
        self.foreground = None;
        self.background = None;
        print!("\x1b[0m");
        Ok(())
    }
}

impl Default for ColorManager {
    fn default() -> Self {
        Self::new()
    }
}
