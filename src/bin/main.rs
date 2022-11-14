use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use hgi80_decoder::{parse_packet, Payload};

fn main() {
    let serial_port = serialport::new("/dev/ttyUSB0", 115200)
        .timeout(Duration::from_millis(2000))
        .open()
        .expect("Failed to open port");

    let mut reader = BufReader::new(serial_port);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(_bytes) =>  {
                match parse_packet(&line.as_str()) {
                    Ok(packet) => {
                        if let Some(Payload::ZoneTemp(zt)) = packet.payload {
                            println!("Temperature {:?}", zt.temperatures);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing line ({})", e);
                        eprintln!("  With line '{}'", line);
                    }
                }
            }
            Err(e) => {
                // Most likely a USB disconnect, restart
                panic!("Error reading from serial port ({})", e);
            }
        }
    }

}