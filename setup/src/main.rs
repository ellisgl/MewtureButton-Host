extern crate serialport;

use std::fs::File;
use std::time::Duration;
use std::io::Write;
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use libpulse_sys::def::pa_port_available_t;
use pulsectl::controllers::DeviceControl;
use pulsectl::controllers::SourceController;
use serialport::SerialPortType;
use serde::Serialize;

#[derive(Debug)]
struct AudioItem {
    value: Option<u32>,
    display_text: String,
}

#[derive(Debug)]
struct SerialPortItem {
    value: Option<String>,
    display_text: String,
}

#[derive(Debug, Serialize)]
struct Config {
    audio_device_id: u32,
    serial_port: String
}

fn main() {
    let mut audio_options: Vec<AudioItem> = vec![];
    let mut serial_options: Vec<SerialPortItem> = vec![];

    // Create a new spinner.
    let pb = ProgressBar::new_spinner();
    pb.set_message("Searching for audio devices...");
    let style = ProgressStyle::default_spinner()
        .tick_chars("|/-\\-")
        .template("{spinner:.green} {msg}")
        .unwrap(); // unwraps the Result container to giv the actual type.
    pb.set_style(style);

    // Start the spinner
    pb.enable_steady_tick(Duration::from_millis(100));
    // thread::sleep(Duration::new(5, 0));
    let mut handler = SourceController::create().unwrap();
    let devices = handler.list_devices().expect("Failed to list devices");

    for dev in devices.clone() {
        if dev.ports.len() == 0 {
            continue;
        }

        let mut found = false;
        for port in dev.ports.clone() {
            if port.available == pa_port_available_t::Unknown || port.available == pa_port_available_t::Yes {
                found = true;
                break;
            }
        }

        if found {
            // available_devices.insert(dev.index, dev.description.unwrap());
            audio_options.push(AudioItem { value: Some(dev.index), display_text: dev.description.unwrap() });
        }
    }
    audio_options.push(AudioItem { value: None, display_text: "Cancel".to_string() });

    pb.finish_and_clear();

    // Get a list of available serial ports
    let pb = ProgressBar::new_spinner();
    pb.set_message("Searching for serial devices...");
    let style = ProgressStyle::default_spinner()
        .tick_chars("|/-\\-")
        .template("{spinner:.green} {msg}")
        .unwrap();
    pb.set_style(style);
    pb.enable_steady_tick(Duration::from_millis(100));
    // thread::sleep(Duration::new(5, 0));
    let ports = serialport::available_ports().expect("Failed to enumerate serial ports");
    let usb_ports: Vec<_> = ports
        .into_iter()
        .filter(|port| match port.port_type {
            SerialPortType::UsbPort(_) => true,
            _ => false,
        })
        .collect();
    if usb_ports.is_empty() {
        println!("No USB serial ports found");
        return;
    } else {
        // println!("Available USB serial ports:");
        // let mut buffer: Vec<u8> = vec![0; 64];
        // let mut bytes_read: usize;

        for port in usb_ports {
            let port_name = port.port_name.clone();
            let serial_port = serialport::new(port_name, 115200)
                .timeout(Duration::from_secs(6))
                .open();

            match serial_port {
                Ok(mut port) => {
                    let mut received_buffer: Vec<u8> = vec![0; 64];
                    let mut read_attempts = 0;

                    loop {
                        if read_attempts >= 5 {
                            // Maximum read attempts reached, break the loop
                            println!("Exceeded maximum read attempts");
                            break;
                        }

                        let bytes_read = match port.read(&mut received_buffer) {
                            Ok(bytes_read) => bytes_read,
                            Err(_error) => {
                                read_attempts += 1;
                                continue;
                            }
                        };

                        if bytes_read > 7 {
                            // Parse the received data
                            let message = ddaa_protocol::parse_protocol_message(&mut received_buffer);
                            if let Some(parsed_message) = message {
                                if parsed_message.command == ddaa_protocol::Command::Ping {
                                    // serial_options.push(SerialPortItem { value: None, display_text: "Cancel".to_string() });
                                    serial_options.push(SerialPortItem { value: Some(port.name().unwrap()), display_text: port.name().unwrap() });
                                    break;
                                }
                            }

                            read_attempts += 1;
                        }
                    }
                },
                Err(error) => {
                    eprintln!("Error reading from serial port: {}", error);
                    continue; // Continue to the next iteration of the loop
                }
            };
        }
    }
    serial_options.push(SerialPortItem { value: None, display_text: "Cancel".to_string() });

    pb.finish_and_clear();

    let audio_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select an audio device")
        .items(&audio_options.iter().map(|item| item.display_text.as_str()).collect::<Vec<_>>())
        .interact()
        .unwrap();

    let selected_audio_item = &audio_options[audio_selection];
    let audio_device = match selected_audio_item.value {
        Some(value) => value,
        None => {
            println!("Program exited");
            return;
        }
    };

    let serial_selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a serial port")
        .items(&serial_options.iter().map(|item| item.display_text.as_str()).collect::<Vec<_>>())
        .interact()
        .unwrap();

    let selected_serial_item = &serial_options[serial_selection];
    let serial = match &selected_serial_item.value {
        Some(serial) => serial,
        None => {
            println!("Program exited");
            return;
        }
    };

    let config = Config {
        audio_device_id: audio_device,
        serial_port: serial.to_string(),
    };
    let toml = toml::to_string(&config).unwrap();
    let mut file = File::create("config.toml").expect("Could not open file.");
    file.write_all(toml.as_bytes()).expect("Could not write TOML config.");
    //println!("{} {}", audio_device, serial);
}
