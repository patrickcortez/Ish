use super::super::error::IshIOError;
use std::os::unix::io::AsRawFd;

const STDIN_FD: i32 = 0;
const STDOUT_FD: i32 = 1;
const STDERR_FD: i32 = 2;

const ICANON: u32 = 0x0002;
const ECHO: u32 = 0x0008;
const TCSANOW: i32 = 0;

#[repr(C)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

extern "C" {
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
}

pub struct UnixStdin {
    fd: i32,
    original_termios: Option<Termios>,
    raw_mode_enabled: bool,
}

impl UnixStdin {
    pub fn new() -> Result<Self, IshIOError> {
        Ok(Self {
            fd: STDIN_FD,
            original_termios: None,
            raw_mode_enabled: false,
        })
    }
    
    pub fn enable_raw_mode(&mut self) -> Result<(), IshIOError> {
        if self.raw_mode_enabled {
            return Ok(());
        }
        
        let mut termios = Termios {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        };
        
        let result = unsafe { tcgetattr(self.fd, &mut termios) };
        if result != 0 {
            return Err(IshIOError::StreamError("Failed to get terminal attributes".to_string()));
        }
        
        self.original_termios = Some(termios);
        
        termios.c_lflag &= !(ICANON | ECHO);
        
        let result = unsafe { tcsetattr(self.fd, TCSANOW, &termios) };
        if result != 0 {
            return Err(IshIOError::StreamError("Failed to set terminal attributes".to_string()));
        }
        
        self.raw_mode_enabled = true;
        Ok(())
    }
    
    pub fn disable_raw_mode(&mut self) -> Result<(), IshIOError> {
        if !self.raw_mode_enabled {
            return Ok(());
        }
        
        if let Some(original) = self.original_termios {
            let result = unsafe { tcsetattr(self.fd, TCSANOW, &original) };
            if result != 0 {
                return Err(IshIOError::StreamError("Failed to restore terminal attributes".to_string()));
            }
        }
        
        self.raw_mode_enabled = false;
        Ok(())
    }
    
    pub fn read_line(&mut self) -> Result<String, IshIOError> {
        let mut buffer = [0u8; 4096];
        let bytes_read = unsafe { read(self.fd, buffer.as_mut_ptr(), buffer.len()) };
        
        if bytes_read < 0 {
            return Err(IshIOError::StreamError("Failed to read from stdin".to_string()));
        }
        
        let input = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
        Ok(input.trim_end_matches('\n').trim_end_matches('\r').to_string())
    }
    
    pub fn read_key(&mut self) -> Result<char, IshIOError> {
        self.enable_raw_mode()?;
        
        let mut buffer = [0u8; 1];
        let bytes_read = unsafe { read(self.fd, buffer.as_mut_ptr(), 1) };
        
        self.disable_raw_mode()?;
        
        if bytes_read <= 0 {
            return Ok(' ');
        }
        
        Ok(buffer[0] as char)
    }
}

impl Default for UnixStdin {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            fd: STDIN_FD,
            original_termios: None,
            raw_mode_enabled: false,
        })
    }
}

pub struct UnixStdout {
    fd: i32,
}

impl UnixStdout {
    pub fn new() -> Result<Self, IshIOError> {
        Ok(Self { fd: STDOUT_FD })
    }
    
    pub fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        let bytes = data.as_bytes();
        let bytes_written = unsafe { write(self.fd, bytes.as_ptr(), bytes.len()) };
        
        if bytes_written < 0 || bytes_written as usize != bytes.len() {
            return Err(IshIOError::StreamError("Failed to write to stdout".to_string()));
        }
        
        Ok(())
    }
    
    pub fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        self.write(data)?;
        self.write("\n")
    }
    
    pub fn flush(&mut self) -> Result<(), IshIOError> {
        Ok(())
    }
}

impl Default for UnixStdout {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self { fd: STDOUT_FD })
    }
}

pub struct UnixStderr {
    fd: i32,
}

impl UnixStderr {
    pub fn new() -> Result<Self, IshIOError> {
        Ok(Self { fd: STDERR_FD })
    }
    
    pub fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        let bytes = data.as_bytes();
        let bytes_written = unsafe { write(self.fd, bytes.as_ptr(), bytes.len()) };
        
        if bytes_written < 0 || bytes_written as usize != bytes.len() {
            return Err(IshIOError::StreamError("Failed to write to stderr".to_string()));
        }
        
        Ok(())
    }
    
    pub fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        self.write(data)?;
        self.write("\n")
    }
    
    pub fn flush(&mut self) -> Result<(), IshIOError> {
        Ok(())
    }
}

impl Default for UnixStderr {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self { fd: STDERR_FD })
    }
}

pub fn enable_virtual_terminal_processing() -> Result<(), IshIOError> {
    Ok(())
}
