extern crate serialport;

use std::error::Error;
use clap::Parser;
use ddaa_protocol::{MessageType, ProtocolMessage};
use home;
use mewture_shared;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::process::exit;
use toml;

use crate::pulseaudio_handler::PulseAudioHandler;
use crate::serial_handler::SerialHandler;

mod serial_handler;
mod pulseaudio_handler;

/// Mewture Button Host Software
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Turn on debug output.
    #[arg(short, long)]
    debug: bool
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let filename = match home::home_dir() {
        Some(path) => {
            path.join(".mewture/config.toml")
        },
        None => {
            return Err("Failed to get home directory.".into());
        }
    };
    let config: mewture_shared::Config = get_config(filename)?;
    if cli.debug {
        // Print the configuration if debug is enabled.
        println!(
            "Config:\n    Device index: {:?}\n    Device name: {:?}\n    Serial port: {:?}\n",
            config.audio_device_index,
            config.audio_device_name,
            config.serial_port
        );
    }

    // Setup PulseAudio.
    let mut pulseaudio = PulseAudioHandler::new(config.audio_device_index)?;

    // Setup the serial port.
    // let mut port = setup_serial_port(&config)?;
    let mut port = SerialHandler::new(&config.serial_port, 115200)?;

    // Get the current mute state.
    let mut current_mute_state = pulseaudio.get_mute_state()?;
    if cli.debug {
        // Print the current mute state if debug is enabled.
        println!("Initial mute state: {:?}", current_mute_state);
    }

    run(&mut pulseaudio, &mut port, &mut current_mute_state, cli.debug)
}

/// Check if the source's mute state has changed.
fn check_for_mute_state_change(
    pulseaudio: &mut PulseAudioHandler,
    port: &mut SerialHandler,
    current_mute_state: &mut bool, debug: bool
) -> Result<(), Box<dyn Error>> {
    // Check if the source mute state has changed.
    let new_mute_state = pulseaudio.get_mute_state()?;
    let mut message = ProtocolMessage {
        message_type: MessageType::Request,
        command: ddaa_protocol::Command::Write,
        variable: 0x00,
        data: vec![0x00]
    };

    if new_mute_state != *current_mute_state {
        message.data[0] = u8::from(new_mute_state);
        return match write_message_to_port(
            port,
            MessageType::Request,
            message,
            debug
        ) {
            Ok(_) => {
                if debug {
                    println!("Setting mute state variable to {:?}", new_mute_state);
                }

                *current_mute_state = new_mute_state;
                Ok(())
            },
            Err(e) => {
                eprintln!("Error writing to serial port: {}", e);
                Err(e.into())
            }
        };
    }

    Ok(())
}

/// Get the configuration.
fn get_config(path: PathBuf) -> Result<mewture_shared::Config, Box<dyn Error>> {
    let content = match read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            return Err(format!("Could not read file `{}`", path.display()).into());
        }
    };

    match toml::from_str(&content) {
        Ok(c) => Ok(c),
        Err(e) =>
            Err(
                format!(
                    "Unable to load data from `{}`. Error: {:?}",
                    path.display(),
                    e
                ).into()
            )
    }
}

/// Handle a read request.
fn handle_read_request(
    port: &mut SerialHandler,
    parsed_message: ProtocolMessage,
    current_mute_state: &bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    // Received read request.
    if debug {
        println!("Received read request");
    }

    if parsed_message.variable == 0x00 {
        // Respond with current mute state.
        if debug {
            println!("Received mute read request");
        }

        let buffer = ddaa_protocol::create_protocol_buffer(
            MessageType::ResponseSuccess,
            ddaa_protocol::Command::Read,
            parsed_message.variable,
            &[u8::from(*current_mute_state)]
        );

        match port.write(&buffer) {
            Ok(size) => {
                if debug {
                    println!("Wrote {} bytes", size);
                    println!("Wrote: {:?}", buffer);
                }
            }
            Err(_e) => {
                eprintln!("Error writing to serial port");
            }
        }
    }

    // Should probably do some sort of error here, but for now just continue on.
    Ok(())
}

/// Handle a request.
fn handle_request(
    pulseaudio: &mut PulseAudioHandler,
    port: &mut SerialHandler,
    parsed_message: ProtocolMessage,
    current_mute_state: &mut bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    if debug {
        println!("Received request: {:?}", parsed_message);
    }

    match parsed_message.command {
        ddaa_protocol::Command::Ping => {
            // Received ping, response to our caller.
            if debug {
                println!("Received ping");
            }

            respond_to_ping(port, parsed_message);
        }
        ddaa_protocol::Command::Read => {
            // Received read request.
            handle_read_request(port, parsed_message, current_mute_state, debug)?
        }
        ddaa_protocol::Command::Write => {
            // Received write request.
            handle_write_request(pulseaudio, port, parsed_message, current_mute_state, debug)?
        }
    }

    Ok(())
}

/// Handle incoming serial data.
fn handle_serial_data(
    port: &mut SerialHandler,
    pulseaudio: &mut PulseAudioHandler,
    received_buffer: &mut [u8],
    current_mute_state: &mut bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    let bytes_read = match port.read(received_buffer) {
        Ok(bytes_read) => bytes_read,
        Err(_e) => { 0 }
    };

    if bytes_read > 7 {
        // Parse the received data.
        let message = ddaa_protocol::parse_protocol_message(received_buffer.to_vec().as_mut());
        if debug {
            // Print the incoming message if debug is enabled.
            println!("Incoming message: {:?}", message);
        }

        if let Some(parsed_message) = message {
            if parsed_message.message_type == MessageType::Request {
                if debug {
                    println!("Received request: {:?}", parsed_message);
                }

                handle_request(
                    pulseaudio,
                    port,
                    parsed_message,
                    current_mute_state,
                    debug
                )?
            }
        }
    } else if bytes_read > 0 && bytes_read <= 7 {
        // We could handle this, but we can just ignore and continue for now.
    }

    Ok(())
}

/// Handle a write request.
fn handle_write_request(
    pulseaudio: &mut PulseAudioHandler,
    port: &mut SerialHandler,
    parsed_message: ProtocolMessage,
    current_mute_state: &mut bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    if debug {
        println!("Received write request: {:?}", parsed_message);
    }

    if parsed_message.variable == 0x00 {
        if debug {
            println!("Received mute write request");
        }

        match parsed_message.data[0] {
            0x00 => {
                // Received mute request.
                // Set source mute state to false.
                if debug {
                    println!("Received mute set to false request");
                }

                pulseaudio.set_mute_state(false)?;
                write_message_to_port(port, MessageType::ResponseSuccess, parsed_message, debug)?;
            }
            0x01 => {
                // Received unmute request.
                // Set source mute state to true.
                if debug {
                    println!("Received mute set to true request");
                }

                pulseaudio.set_mute_state(true)?;
                write_message_to_port(port, MessageType::ResponseSuccess, parsed_message, debug)?;
            }
            0x02 => {
                // Received invert mute request.
                if debug {
                    println!("Received mute set to invert request");
                }

                pulseaudio.set_mute_state(!*current_mute_state)?;
                write_message_to_port(port, MessageType::ResponseSuccess, parsed_message, debug)?;

            }
            _ => {
                // Data for mute variable is invalid.
                eprintln!("Received unknown value: {:?}", parsed_message.data[0]);
                write_message_to_port(port, MessageType::ResponseError, parsed_message, debug)?;
            }
        }
    }

    Ok(())
}

/// Respond to a ping message.
fn respond_to_ping(port: &mut SerialHandler, message: ProtocolMessage) {
    match port.write(&ddaa_protocol::create_protocol_buffer(
        MessageType::ResponseSuccess,
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

/// The loop that does all the things for the daemon.
fn run(
    pulseaudio: &mut PulseAudioHandler,
    port: &mut SerialHandler,
    current_mute_state: &mut bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    let mut received_buffer: Vec<u8> = vec![0; 64];
    loop {
        // Handle incoming serial data.
        handle_serial_data(port, pulseaudio, &mut received_buffer, current_mute_state, debug)?;

        // Check if the source mute state has changed.
        check_for_mute_state_change(pulseaudio, port, current_mute_state, debug)?;

        // Clear the buffer.
        received_buffer.clear();
        received_buffer.resize(64, 0);
    }
}

/// Write a message to the serial port.
fn write_message_to_port(
    port: &mut SerialHandler,
    message_type: MessageType,
    parsed_message: ProtocolMessage,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    let buffer = &ddaa_protocol::create_protocol_buffer(
        message_type,
        parsed_message.command,
        parsed_message.variable,
        &parsed_message.data
    );
    match port.write(buffer) {
        Ok(size) => {
            if debug {
                println!("write_response_to_port responded with {:?} bytes", size);
                println!("write_response_to_port wrote {:?}", buffer);
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("Error writing to serial port: {}", e);
            Err(e.into())
        }
    }
}
