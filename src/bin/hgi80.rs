use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use reqwest::header::CONTENT_TYPE;
use std::io::BufReader;
use anyhow::{Result, Context};

struct HGI80 {
    reader:  BufReader<serialport>,
    endpoint: String,
    keep_running: Arc<AtomicBool>
    handle: thread::JoinHandle,
}

impl HGI80 {
    fn new(port: &str, endpoint: &str, keep_running: &AtomicBool) -> Result<Self>
    {
        let serial_port = serialport::new(port, 115200)
            .timeout(Duration::from_millis(500))
            .open()
            .with_context(|| "HGI80 serial port");

        let instance = Self {
            reader: BufReader::new(serial_port),
            endpoint,
            keep_running: Arc::new(AtomicBool::new(true)),
        };

        let keep_running = instance.keep_running.clone();
        thread::spawn(|| )
    }
    
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
        let res = client
            .post(endpoint)
            .header(CONTENT_TYPE, "application/json")
            .body(payload)
            .send();

        if let Err(e) = res {
            eprintln!("Failed to post data to {} ({})", endpoint, e);
        } else {
            println!("Response: {}", res.unwrap().text().unwrap_or_default());
        }
    }

    /// Thread loop reading data from the hgi
    fn evohome_loop(port: &str, endpoint: &str)
    {
        let mut reader = BufReader::new(serial_port);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(_bytes) => match parse_packet(line.as_str()) {
                    Ok(packet) => {
                        // Packets with an RSSI higher than 80 usually have lots of bit errors
                        if packet.rssi < 80 { 
                            if let Some(Payload::ZoneTemp(zt)) = packet.payload {
                                post_temperature_data(&args.endpoint, &zt.temperatures);

                                println!("Temperature {:?}", zt.temperatures);
                                if let Err(e) = writeln!(file, "{}", line.as_str()) {
                                    eprintln!("Couldn't write to file: {}", e);
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        //eprintln!("Error parsing line ({:#})", e);
                        //eprintln!("  With line '{}'", line);
                    }
                },
                Err(e) => {
                    match e.kind() {
                        std::io::ErrorKind::TimedOut => eprintln!("Error reading from serial port ({:#})", e),
                        _ => panic!("Error reading from serial port ({:#})", e),
                    }
                }
            }
        }
    }
}
