use serde::{ Deserialize, Serialize };

/// Configuration to store the audio device and serial port information.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub audio_device_name: String,
    pub audio_device_index: u32,
    pub serial_port: String
}
