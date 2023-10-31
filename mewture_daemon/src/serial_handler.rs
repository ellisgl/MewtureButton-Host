extern crate serialport;

use serialport::SerialPort;
use std::error::Error;
use std::io::Write;
use std::time::Duration;

pub struct SerialHandler {
    port: Box<dyn SerialPort>
}

// Abstract the serialport crate for future testing?
impl SerialHandler {
    pub fn new(port_path: &str, baud_rate: u32) -> Result<Self, Box<dyn Error>> {
        let port = serialport::new(port_path, baud_rate)
            .timeout(Duration::from_millis(300))
            .open()?;

        Ok(Self { port })
    }

    // Replicate the base functionality of serialport.
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Box<dyn Error>> {
        let bytes_read = self.port.read(buffer)?;
        Ok(bytes_read)
    }

    pub fn write(&mut self, buffer: &[u8]) -> Result<usize, Box<dyn Error>> {
        let bytes_written = self.port.write(buffer)?;
        Ok(bytes_written)
    }

    // Return the name (path) of the serialport.
    pub fn get_name(&mut self) -> Option<String> {
        self.port.name()
    }
}
