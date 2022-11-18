use reqwest;
use reqwest::header::CONTENT_TYPE;
use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use hgi80_decoder::{parse_packet, Payload};
use std::collections::HashMap;

fn post_temperature_data(endpoint: &str, data: &HashMap<u8, f32>) {
    let mut payload: String = String::from("[");
    for (key, value) in data {
        if payload.len() > 1 {
            payload += ",";    
        }
        payload += &format!("{{\"id\":\"RADIATOR{}\", \"temp\":{} }}", key, value);
    }
    payload += "]";
    println!("PAYLOAD {}", &payload);

    let client = reqwest::blocking::Client::new();
    let res = client.post(endpoint)
        .header(CONTENT_TYPE, "application/json")
        .body(payload)
        .send();

    if let Err(e) = res {
        eprintln!("Failed to post data to {} ({})", endpoint, e);
    }
    else {
        println!("Response: {}", res.unwrap().text().unwrap_or_default());
    }
}

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
                            post_temperature_data("http://xxxx", &zt.temperatures);
                            println!("Temperature {:?}", zt.temperatures);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error parsing line ({:#})", e);
                        eprintln!("  With line '{}'", line);
                    }
                }
            }
            Err(e) => {
                // Most likely a USB disconnect, restart
                panic!("Error reading from serial port ({:#})", e);
            }
        }
    }
}
