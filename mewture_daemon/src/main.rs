extern crate serialport;

use std::error::Error;
use clap::Parser;
use ddaa_protocol::{MessageType, ProtocolMessage};
use home;
use mewture_shared;
use pulser::api::PAIdent;
use pulser::simple::PulseAudio;
use serialport::SerialPort;
use std::fs::read_to_string;
use std::path::PathBuf;
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
    let mut pa = setup_pulseaudio(&config)?;

    // Setup the serial port.
    let mut port = setup_serial_port(&config)?;

    // Get the current mute state.
    let mut current_mute_state = get_source_mute(&pa, &config)?;
    if cli.debug {
        // Print the current mute state if debug is enabled.
        println!("Initial mute state: {:?}", current_mute_state);
    }

    let mut received_buffer: Vec<u8> = vec![0; 64];
    loop {
        let bytes_read = match port.read(&mut received_buffer) {
            Ok(bytes_read) => bytes_read,
            Err(_e) => { 0 }
        };

        if bytes_read > 7 {
            // Parse the received data.
            let message = ddaa_protocol::parse_protocol_message(&mut received_buffer);
            if cli.debug {
                // Print the incoming message if debug is enabled.
                println!("Incoming message: {:?}", message);
            }

            if let Some(parsed_message) = message {
                if parsed_message.message_type == MessageType::Request {
                    if cli.debug {
                        println!("Received request: {:?}", parsed_message);
                    }

                    match parsed_message.command {
                        ddaa_protocol::Command::Ping => {
                            // Received ping, response to our caller.
                            if cli.debug {
                                println!("Received ping");
                            }

                            respond_to_ping(&mut port, parsed_message);
                        }
                        ddaa_protocol::Command::Read => {
                            // Received read request.
                            if cli.debug {
                                println!("Received read request");
                            }

                            if parsed_message.variable == 0x00 {
                                // Respond with current mute state.
                                if cli.debug {
                                    println!("Received mute read request");
                                }

                                let buffer = ddaa_protocol::create_protocol_buffer(
                                    MessageType::ResponseSuccess,
                                    ddaa_protocol::Command::Read,
                                    parsed_message.variable,
                                    &[u8::from(current_mute_state)]
                                );

                                match port.write(&buffer) {
                                    Ok(size) => {
                                        if cli.debug {
                                            println!("Wrote {} bytes", size);
                                            println!("Wrote: {:?}", buffer);
                                        }
                                    }
                                    Err(_e) => {
                                        eprintln!("Error writing to serial port");
                                    }
                                }
                            }
                        }
                        ddaa_protocol::Command::Write => {
                            // Received write request.
                            if cli.debug {
                                println!("Received write request");
                            }

                            if parsed_message.variable == 0x00 {
                                if cli.debug {
                                    println!("Received mute write request");
                                }

                                match parsed_message.data[0] {
                                    0x00 => {
                                        // Received mute request.
                                        // Set source mute state to true.
                                        if cli.debug {
                                            println!("Received mute set to true request");
                                        }

                                        set_source_mute(&pa, &config, false, cli.debug)?;
                                        write_message_to_port(&mut port, MessageType::ResponseSuccess, parsed_message, cli.debug)?;
                                    }
                                    0x01 => {
                                        // Received unmute request.
                                        // Set source mute state to false.
                                        if cli.debug {
                                            println!("Received mute set to false request");
                                        }

                                        set_source_mute(&pa, &config, true, cli.debug)?;
                                        write_message_to_port(&mut port, MessageType::ResponseSuccess, parsed_message, cli.debug)?;
                                    }
                                    0x02 => {
                                        // Received invert mute request.
                                        if cli.debug {
                                            println!("Received mute set to invert request");
                                        }

                                        invert_source_mute(&mut pa, &config, &mut port, &parsed_message, current_mute_state, cli.debug)?;
                                        write_message_to_port(&mut port, MessageType::ResponseSuccess, parsed_message, cli.debug)?;

                                    }
                                   _ => {
                                       // Data for mute variable is invalid.
                                       eprintln!("Received unknown value:{:?}", parsed_message.data[0]);
                                       write_message_to_port(&mut port, MessageType::ResponseError, parsed_message, cli.debug)?;
                                    }
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
        let new_mute_state = get_source_mute(&pa, &config)?;
        let mut message = ProtocolMessage {
            message_type: MessageType::Request,
            command: ddaa_protocol::Command::Write,
            variable: 0x00,
            data: vec![0x00]
        };

        if new_mute_state != current_mute_state {
            message.data[0] = u8::from(new_mute_state);
            match write_message_to_port(
                &mut port,
                MessageType::Request,
                message,
                cli.debug
            ) {
                Ok(_) => {
                    if cli.debug {
                        println!("Setting mute state variable to {:?}", new_mute_state);
                    }

                    current_mute_state = new_mute_state;
                },
                Err(e) => {
                    eprintln!("Error writing to serial port: {}", e);
                    exit(1);
                }
            };
        }

        // Clear the buffer.
        received_buffer.clear();
        received_buffer.resize(64, 0);
    }
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

/// Set the source mute state as the inverted value of it's current state.
fn invert_source_mute(
    pa: &mut PulseAudio,
    config: &mewture_shared::Config,
    port: &mut Box<dyn SerialPort>,
    parsed_message: &ProtocolMessage,
    mute_state: bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    if debug {
        println!("Inverting mute state from {:?} to {:?}", mute_state, !mute_state);
    }

    match set_source_mute(pa, config, !mute_state, debug) {
        Ok(_) => {
            // Responds with success message.
            let buffer = &ddaa_protocol::create_protocol_buffer(
                MessageType::ResponseSuccess,
                ddaa_protocol::Command::Write,
                parsed_message.variable,
                &[u8::from(!mute_state)]
            );
            match port.write(
                buffer,
            ) {
                Ok(size) => {
                    if debug {
                        println!("set_source_mute responded with {:?} bytes: {:?}", size, buffer);
                    }

                    Ok(())
                },
                Err(e) => {
                    eprintln!("Error writing to serial port: {}", e);
                    Err(e.into())
                }
            }
        },
        Err(e) => {
            eprintln!("Error inverting mute: {}", e);
            Err(e.into())
        }
    }
}

/// Respond to a ping message.
fn respond_to_ping(port: &mut Box<dyn SerialPort>, message: ProtocolMessage) {
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

/// Set the default source device in PulseAudio.
fn setup_default_source(pa: &PulseAudio, config: &mewture_shared::Config) -> Result<(), Box<dyn Error>> {
    match pa.set_default_source(PAIdent::Index(config.audio_device_index)) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error setting default source: {:?}", e);
            Err(e.into())
        }
    }
}

/// Connect and setup the source device.
fn setup_pulseaudio(config: &mewture_shared::Config) -> Result<PulseAudio, Box<dyn Error>> {
    let pa = PulseAudio::connect(Some("Mewture Button"));
    setup_default_source(&pa, config)?;

    Ok(pa)
}

/// Open and configure the serial port.
fn setup_serial_port(config: &mewture_shared::Config) -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
    // Check for the serial device.
    let serial_port = serialport::new(&config.serial_port, 115200)
        .timeout(Duration::from_millis(300))
        .open();

    match serial_port {
        Ok(p) => Ok(p),
        Err(e) => {
            eprintln!("Error reading from serial port: {}", e);
            Err(e.into())
        }
    }
}

/// Get the source mute state.
fn get_source_mute(pa: &PulseAudio, config: &mewture_shared::Config) -> Result<bool, Box<dyn Error>> {
    match pa.get_source_mute(PAIdent::Index(config.audio_device_index)) {
        Ok(m) => Ok(m),
        Err(e) => {
            eprintln!("Error getting mute state: {}", e);
            Err(e.into())
        }
    }
}

/// Set the source mute state.
fn set_source_mute(
    pa: &PulseAudio,
    config: &mewture_shared::Config,
    mute_state: bool,
    debug: bool
) -> Result<(), Box<dyn Error>> {
    if debug {
        println!("Setting source mute state to {:?}", mute_state);
    }

    match pa.set_source_mute(PAIdent::Index(config.audio_device_index), mute_state) {
        Ok(res) => {
            if debug {
                println!("pa.set_source_mute responded with {:?}", res);
            }

            Ok(())
        },
        Err(e) => {
            eprintln!("Error setting mute state: {}", e);
            Err(e.into())
        }
    }
}

/// Write a message to the serial port.
fn write_message_to_port(
    port: &mut Box<dyn SerialPort>,
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
