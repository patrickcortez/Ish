use super::super::error::IshIOError;
use std::ptr;

const STD_INPUT_HANDLE: u32 = -10i32 as u32;
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_ERROR_HANDLE: u32 = -12i32 as u32;

const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
const ENABLE_LINE_INPUT: u32 = 0x0002;
const ENABLE_ECHO_INPUT: u32 = 0x0004;
const ENABLE_PROCESSED_INPUT: u32 = 0x0001;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(nStdHandle: u32) -> *mut ();
    fn WriteFile(
        hFile: *mut (),
        lpBuffer: *const u8,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut (),
    ) -> i32;
    fn ReadFile(
        hFile: *mut (),
        lpBuffer: *mut u8,
        nNumberOfBytesToRead: u32,
        lpNumberOfBytesRead: *mut u32,
        lpOverlapped: *mut (),
    ) -> i32;
    fn FlushFileBuffers(hFile: *mut ()) -> i32;
    fn GetConsoleMode(hConsoleHandle: *mut (), lpMode: *mut u32) -> i32;
    fn SetConsoleMode(hConsoleHandle: *mut (), dwMode: u32) -> i32;
}

fn get_std_handle(handle_type: u32) -> Result<*mut (), IshIOError> {
    let handle = unsafe { GetStdHandle(handle_type) };
    if handle.is_null() || handle as isize == -1 {
        return Err(IshIOError::StreamError("Failed to get standard handle".to_string()));
    }
    Ok(handle)
}

pub struct WindowsStdin {
    handle: *mut (),
    original_mode: u32,
    raw_mode_enabled: bool,
}

impl WindowsStdin {
    pub fn new() -> Result<Self, IshIOError> {
        let handle = get_std_handle(STD_INPUT_HANDLE)?;
        let mut mode: u32 = 0;
        let result = unsafe { GetConsoleMode(handle, &mut mode) };
        let original_mode = if result != 0 { mode } else { 0 };
        
        Ok(Self {
            handle,
            original_mode,
            raw_mode_enabled: false,
        })
    }
    
    pub fn enable_raw_mode(&mut self) -> Result<(), IshIOError> {
        if self.raw_mode_enabled {
            return Ok(());
        }
        
        let mut mode: u32 = 0;
        let result = unsafe { GetConsoleMode(self.handle, &mut mode) };
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to get console mode".to_string()));
        }
        
        let new_mode = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT);
        let result = unsafe { SetConsoleMode(self.handle, new_mode) };
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to set console mode".to_string()));
        }
        
        self.raw_mode_enabled = true;
        Ok(())
    }
    
    pub fn disable_raw_mode(&mut self) -> Result<(), IshIOError> {
        if !self.raw_mode_enabled {
            return Ok(());
        }
        
        let result = unsafe { SetConsoleMode(self.handle, self.original_mode) };
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to restore console mode".to_string()));
        }
        
        self.raw_mode_enabled = false;
        Ok(())
    }
    
    pub fn read_line(&mut self) -> Result<String, IshIOError> {
        let mut buffer = [0u8; 4096];
        let mut bytes_read: u32 = 0;
        
        let result = unsafe {
            ReadFile(
                self.handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut bytes_read,
                ptr::null_mut(),
            )
        };
        
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to read from stdin".to_string()));
        }
        
        let input = String::from_utf8_lossy(&buffer[..bytes_read as usize]);
        Ok(input.trim_end_matches('\n').trim_end_matches('\r').to_string())
    }
    
    pub fn read_key(&mut self) -> Result<char, IshIOError> {
        self.enable_raw_mode()?;
        
        let mut buffer = [0u8; 1];
        let mut bytes_read: u32 = 0;
        
        let result = unsafe {
            ReadFile(
                self.handle,
                buffer.as_mut_ptr(),
                1,
                &mut bytes_read,
                ptr::null_mut(),
            )
        };
        
        self.disable_raw_mode()?;
        
        if result == 0 || bytes_read == 0 {
            return Ok(' ');
        }
        
        Ok(buffer[0] as char)
    }
}

impl Default for WindowsStdin {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            handle: ptr::null_mut(),
            original_mode: 0,
            raw_mode_enabled: false,
        })
    }
}

pub struct WindowsStdout {
    handle: *mut (),
}

impl WindowsStdout {
    pub fn new() -> Result<Self, IshIOError> {
        let handle = get_std_handle(STD_OUTPUT_HANDLE)?;
        Ok(Self { handle })
    }
    
    pub fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        let bytes = data.as_bytes();
        let mut bytes_written: u32 = 0;
        
        let result = unsafe {
            WriteFile(
                self.handle,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut bytes_written,
                ptr::null_mut(),
            )
        };
        
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to write to stdout".to_string()));
        }
        
        Ok(())
    }
    
    pub fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        self.write(data)?;
        self.write("\n")
    }
    
    pub fn flush(&mut self) -> Result<(), IshIOError> {
        let result = unsafe { FlushFileBuffers(self.handle) };
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to flush stdout".to_string()));
        }
        Ok(())
    }
}

impl Default for WindowsStdout {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            handle: ptr::null_mut(),
        })
    }
}

pub struct WindowsStderr {
    handle: *mut (),
}

impl WindowsStderr {
    pub fn new() -> Result<Self, IshIOError> {
        let handle = get_std_handle(STD_ERROR_HANDLE)?;
        Ok(Self { handle })
    }
    
    pub fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        let bytes = data.as_bytes();
        let mut bytes_written: u32 = 0;
        
        let result = unsafe {
            WriteFile(
                self.handle,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut bytes_written,
                ptr::null_mut(),
            )
        };
        
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to write to stderr".to_string()));
        }
        
        Ok(())
    }
    
    pub fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        self.write(data)?;
        self.write("\n")
    }
    
    pub fn flush(&mut self) -> Result<(), IshIOError> {
        let result = unsafe { FlushFileBuffers(self.handle) };
        if result == 0 {
            return Err(IshIOError::StreamError("Failed to flush stderr".to_string()));
        }
        Ok(())
    }
}

impl Default for WindowsStderr {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            handle: ptr::null_mut(),
        })
    }
}

pub fn enable_virtual_terminal_processing() -> Result<(), IshIOError> {
    let handle = get_std_handle(STD_OUTPUT_HANDLE)?;
    let mut mode: u32 = 0;
    let result = unsafe { GetConsoleMode(handle, &mut mode) };
    
    if result != 0 {
        let new_mode = mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
        let result = unsafe { SetConsoleMode(handle, new_mode) };
        if result == 0 {
            return Err(IshIOError::TerminalError("Failed to enable virtual terminal processing".to_string()));
        }
    }
    
    Ok(())
}
