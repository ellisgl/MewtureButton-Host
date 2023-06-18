extern crate serialport;

use clap::Parser;
use home;
use mewture_shared;
use pulser::api::PAIdent;
use pulser::simple::PulseAudio;
use serialport::SerialPort;
use std::fs::read_to_string;
use std::process::exit;
use std::time::Duration;
use toml;

/// Mewture Button Host Software
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Turn on debug output.
    #[arg(short, long)]
    debug: bool
}

fn main() {
    let cli = Cli::parse();
    let filename = match home::home_dir() {
        Some(path) => {
            path.join(".mewture/config.toml")
        },
        None => {
            eprintln!("Failed to get home directory.");
            exit(1);
        }
    };

    let content = match read_to_string(&filename) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Could not read file `{}`", filename.display());
            exit(1);
        }
    };

    let config: mewture_shared::Config = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Unable to load data from `{}`. Error: {:?}", filename.display(), e);
            exit(1);
        }
    };

    if cli.debug {
        println!(
            "Config:\n    Device index: {:?}\n    Device name: {:?}\n    Serial port: {:?}\n",
            config.audio_device_index,
            config.audio_device_name,
            config.serial_port
        );
    }

    let pa = PulseAudio::connect(Some("Mewture Button"));
    match pa.set_default_source(PAIdent::Index(config.audio_device_index)) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error setting default source: {:?}", e);
            exit(1)
        }
    };

    // Check for the serial device.
    let serial_port = serialport::new(config.serial_port, 115200)
        .timeout(Duration::from_millis(300))
        .open();

    let mut port = match serial_port {
        Ok(p) => p,
        Err(error) => {
            eprintln!("Error reading from serial port: {}", error);
            exit(1); // Continue to the next iteration of the loop
        }
    };

    let mut mute_state = match pa.get_source_mute(PAIdent::Index(config.audio_device_index)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error getting mute state: {}", e);
            exit(1)
        }
    };

    let mut received_buffer: Vec<u8> = vec![0; 64];
    loop {
        let bytes_read = match port.read(&mut received_buffer) {
            Ok(bytes_read) => bytes_read,
            Err(_e) => { 0 }
        };

        if bytes_read > 7 {
            // Parse the received data
            let message = ddaa_protocol::parse_protocol_message(&mut received_buffer);
            if cli.debug {
                println!("Incoming message: {:?}", message);
            }

            if let Some(parsed_message) = message {
                if parsed_message.message_type == ddaa_protocol::MessageType::Request {
                    match parsed_message.command {
                        ddaa_protocol::Command::Ping => {
                            // Received ping, response to our caller.
                            respond_to_ping(&mut port, parsed_message);
                        }
                        ddaa_protocol::Command::Read => {
                            // Received read request.
                            if parsed_message.variable == 0x00 {
                                // Respond with current mute state.
                                match port.write(&ddaa_protocol::create_protocol_buffer(
                                    ddaa_protocol::MessageType::ResponseSuccess,
                                    ddaa_protocol::Command::Read,
                                    parsed_message.variable,
                                    &[u8::from(!mute_state)],
                                )) {
                                    Ok(_) => {}
                                    Err(_e) => {
                                        eprintln!("Error writing to serial port");
                                    }
                                }
                            }
                        }
                        ddaa_protocol::Command::Write => {
                            // Received write request.
                            if parsed_message.variable == 0x00 {
                                match parsed_message.data[0] {
                                    0x00 => {
                                        // Received mute request.
                                        // Set source mute state to true.
                                        match pa.set_source_mute(
                                            PAIdent::Index(config.audio_device_index),
                                            false,
                                        ) {
                                            Ok(_) => {
                                                // We don't have this implemented yet on the hardware side.
                                            }
                                            Err(e) => {
                                                eprintln!("Error un-muting: {}", e);
                                                exit(1);
                                            }
                                        }
                                    }
                                    0x01 => {
                                        // Received unmute request.
                                        // Set source mute state to false.
                                        match pa.set_source_mute(
                                            PAIdent::Index(config.audio_device_index),
                                            true,
                                        ) {
                                            Ok(_) => {
                                                // We don't have this implemented yet on the hardware side.
                                            }
                                            Err(e) => {
                                                eprintln!("Error muting: {}", e);
                                                exit(1);
                                            }
                                        }
                                    }
                                    0x02 => {
                                        // Received invert mute request.
                                        // Set the source mute state to the opposite of the current state.
                                        match pa.set_source_mute(
                                            PAIdent::Index(config.audio_device_index),
                                            !mute_state,
                                        ) {
                                            Ok(_) => {
                                                // Responds with success message.
                                                match port.write(
                                                    &ddaa_protocol::create_protocol_buffer(
                                                        ddaa_protocol::MessageType::ResponseSuccess,
                                                        ddaa_protocol::Command::Write,
                                                        parsed_message.variable,
                                                        &[u8::from(!mute_state)],
                                                    ),
                                                ) {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        eprintln!("Error writing to serial port: {}", e);
                                                        exit(1);
                                                    }
                                                };
                                            }
                                            Err(e) => {
                                                eprintln!("Error inverting mute: {}", e);
                                                exit(1);
                                            }
                                        }
                                    }
                                    3_u8..=u8::MAX => {
                                        eprintln!(
                                            "Received unknown value:{:?}",
                                            parsed_message.data[0]
                                        );
                                    }
                                }
                                // Data for mute variable is invalid.
                                if parsed_message.data[0] > 0x02 {
                                    write_response_to_port(&mut port, ddaa_protocol::MessageType::ResponseError, parsed_message);
                                } else {
                                    // We have a valid request, respond with success.
                                    write_response_to_port(&mut port, ddaa_protocol::MessageType::ResponseSuccess, parsed_message);
                                }
                            }
                        }
                    }
                }
            }
        } else if bytes_read > 0 && bytes_read <= 7 {
            // We could handle this, but we can just ignore and continue for now.
        }

        // Check if the source mute state has changed.
        match pa.get_source_mute(PAIdent::Index(config.audio_device_index)) {
            Ok(m) => {
                if !mute_state == m {
                    // Source mute state has changed, send a write request to the device.
                    match port.write(&ddaa_protocol::create_protocol_buffer(
                        ddaa_protocol::MessageType::Request,
                        ddaa_protocol::Command::Write,
                        0x00,
                        &[u8::from(mute_state)]
                    )) {
                        Ok(_) => {
                            // Update store mute state to match source's mute state.
                            mute_state = m;
                        },
                        Err(e) => {
                            // Something bad happened, let's just exit with an error..
                            eprintln!("Error writing to serial port: {}", e);
                            exit(1);
                        }
                    }
                }
            },
            Err(e) => {
                // Something bad happened, let's just exit with an error.
                eprintln!("Error getting mute state: {}", e);
                exit(1);
            }
        }

        // Clear the buffer.
        received_buffer.clear();
        received_buffer.resize(64, 0);
    }
}

// Respond to a ping message.
fn respond_to_ping(port: &mut Box<dyn SerialPort>, message: ddaa_protocol::ProtocolMessage) {
    match port.write(&ddaa_protocol::create_protocol_buffer(
        ddaa_protocol::MessageType::ResponseSuccess,
        ddaa_protocol::Command::Ping,
        message.variable,
        &message.data,
    )) {
        Ok(_) => { },
        Err(e) => {
            // Something bad happened, let's just exit with an error.
            eprintln!("Error writing to serial port: {}", e);
            exit(1);
        }
    };
}

fn write_response_to_port(
    port: &mut Box<dyn SerialPort>,
    message_type: ddaa_protocol::MessageType,
    parsed_message: ddaa_protocol::ProtocolMessage
) {
    match port.write(&ddaa_protocol::create_protocol_buffer(
        message_type,
        parsed_message.command,
        parsed_message.variable,
        &parsed_message.data,
    )) {
        Ok(_) => {
            // We could respond with an error, but it's not implemented in firmware.
        }
        Err(e) => {
            eprintln!("Error writing to serial port: {}", e);
            // Not going to exit, let's just continue for S&Gs.
        }
    }
}
