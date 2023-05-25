extern crate serialport;

use std::{thread, time};
// use std::collections::HashMap;
use std::time::Duration;

use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use libpulse_sys::def::pa_port_available_t;
use pulsectl::controllers::DeviceControl;
use pulsectl::controllers::SourceController;
use serialport::SerialPortType;
// use ding_ding_ack_ack::parse_protocol_message;

#[derive(Debug)]
struct AudioItem {
    value: Option<u32>,
    display_text: String,
}

struct SerialPortItem {
    value: Option<String>,
    display_text: String,
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
    thread::sleep(time::Duration::new(5, 0));
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
    thread::sleep(time::Duration::new(5, 0));
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
    } else {
        // println!("Available USB serial ports:");
        let mut buffer: Vec<u8> = vec![0; 64];
        let mut bytes_read: usize;

        for port in usb_ports {
            let port_name = port.port_name.clone();
            let serial_port = serialport::new(port_name, 115200)
                .timeout(std::time::Duration::from_secs(5))
                .open();
            match serial_port {
                Ok(mut port) => {
                    // Read data from the serial port
                    match port.read(&mut buffer) {
                        Ok(bytes_available) => {
                            bytes_read = bytes_available;
                            bytes_available
                        },
                        Err(_error) => {
                            continue;
                        }
                    };
                },
                Err(_error) => {
                    continue;
                }
            };

            if bytes_read > 0 {
                // Parse the received data
                let message = ding_ding_ack_ack::parse_protocol_message(&buffer[0..bytes_read]);
                if let Some(parsed_message) = message: ding_ding_ack_ack::Message {
                    println!("Parsed message: {:?}", parsed_message);
                } else {
                    println!("Invalid message received");
                }
            }

            // Clear the buffer for the next read
            buffer.clear();
            buffer.resize(64, 0);
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
    match selected_audio_item.value {
        Some(value) => println!("Selected audio device: {}, {}", value, selected_audio_item.display_text),
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
    match &selected_serial_item.value {
        Some(value) => println!("Selected serial port: {}", value),
        None => {
            println!("Program exited");
            return;
        }
    }
}

// use ding_ding_ack_ack;
// fn main() {
//     let port_name = "/dev/ttyUSB0";
//     let serial_port = serialport::new(port_name, 115200)
//         .timeout(std::time::Duration::from_secs(5))
//         .open();
//     match serial_port {
//         Ok(mut port) => {
//             let mut buffer: Vec<u8> = vec![0; 64];

//             loop {
//                 // Read data from the serial port
//                 let bytes_read = match port.read(&mut buffer) {
//                     Ok(bytes_read) => bytes_read,
//                     Err(error) => {
//                         eprintln!("Error reading from serial port: {}", error);
//                         // break;
//                         continue;
//                     }
//                 };

//                 // Parse the received data
//                 let message = ding_ding_ack_ack::parse_protocol_message(&buffer[0..bytes_read]);
//                 if let Some(parsed_message) = message {
//                     // Process the parsed message
//                     println!("Parsed message: {:?}", parsed_message);
//                 } else {
//                     println!("Invalid message received");
//                 }

//                 // Clear the buffer for the next read
//                 buffer.clear();
//                 buffer.resize(64, 0);
//             }
//         }
//         Err(error) => {
//             eprintln!("Failed to open serial port: {}", error);
//         }
//     }
// }
