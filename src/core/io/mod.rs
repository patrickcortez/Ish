pub mod error;
pub mod color;
pub mod platform;

pub use error::IshIOError;
pub use color::{Color, ColorManager};

pub trait InputStream {
    fn read_line(&mut self) -> Result<String, IshIOError>;
    fn read_key(&mut self) -> Result<char, IshIOError>;
}

pub trait OutputStream {
    fn write(&mut self, data: &str) -> Result<(), IshIOError>;
    fn write_line(&mut self, data: &str) -> Result<(), IshIOError>;
    fn flush(&mut self) -> Result<(), IshIOError>;
}

pub struct StdinStream {
    #[cfg(windows)]
    inner: platform::WindowsStdin,
    #[cfg(unix)]
    inner: platform::UnixStdin,
}

impl StdinStream {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            inner: platform::WindowsStdin::new().unwrap_or_default(),
            #[cfg(unix)]
            inner: platform::UnixStdin::new().unwrap_or_default(),
        }
    }
}

impl Default for StdinStream {
    fn default() -> Self {
        Self::new()
    }
}

impl InputStream for StdinStream {
    fn read_line(&mut self) -> Result<String, IshIOError> {
        #[cfg(windows)]
        return self.inner.read_line();
        #[cfg(unix)]
        return self.inner.read_line();
    }
    
    fn read_key(&mut self) -> Result<char, IshIOError> {
        #[cfg(windows)]
        return self.inner.read_key();
        #[cfg(unix)]
        return self.inner.read_key();
    }
}

pub struct StdoutStream {
    #[cfg(windows)]
    inner: platform::WindowsStdout,
    #[cfg(unix)]
    inner: platform::UnixStdout,
}

impl StdoutStream {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            inner: platform::WindowsStdout::new().unwrap_or_default(),
            #[cfg(unix)]
            inner: platform::UnixStdout::new().unwrap_or_default(),
        }
    }
}

impl Default for StdoutStream {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStream for StdoutStream {
    fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.write(data);
        #[cfg(unix)]
        return self.inner.write(data);
    }
    
    fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.write_line(data);
        #[cfg(unix)]
        return self.inner.write_line(data);
    }
    
    fn flush(&mut self) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.flush();
        #[cfg(unix)]
        return self.inner.flush();
    }
}

pub struct StderrStream {
    #[cfg(windows)]
    inner: platform::WindowsStderr,
    #[cfg(unix)]
    inner: platform::UnixStderr,
}

impl StderrStream {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            inner: platform::WindowsStderr::new().unwrap_or_default(),
            #[cfg(unix)]
            inner: platform::UnixStderr::new().unwrap_or_default(),
        }
    }
}

impl Default for StderrStream {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStream for StderrStream {
    fn write(&mut self, data: &str) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.write(data);
        #[cfg(unix)]
        return self.inner.write(data);
    }
    
    fn write_line(&mut self, data: &str) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.write_line(data);
        #[cfg(unix)]
        return self.inner.write_line(data);
    }
    
    fn flush(&mut self) -> Result<(), IshIOError> {
        #[cfg(windows)]
        return self.inner.flush();
        #[cfg(unix)]
        return self.inner.flush();
    }
}
