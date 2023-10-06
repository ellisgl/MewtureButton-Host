use serde::{ Deserialize, Serialize };

/// Configuration to store the audio device and serial port information.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub audio_device_name: String,
    pub serial_port: String
}
