use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::collections::HashMap;
use reqwest::header::CONTENT_TYPE;
use std::io::BufReader;
use anyhow::{bail, format_err, Result, Context};
use std::io::BufRead;


pub struct HGI80 {
    keep_running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl HGI80 {
    pub fn new(port: &str, endpoint: &str) -> Result<Self>
    {
        let serial_port = serialport::new(port, 115200)
            .timeout(Duration::from_millis(500))
            .open()
            .expect("HGI80 serial port");

        let keep_running = Arc::new(AtomicBool::new(true));

        let keep_running_thread = keep_running.clone();
        let endpoint_thread = endpoint.to_string();

        let handle = thread::spawn(move || {
            Self::evohome_loop(&mut BufReader::new(serial_port), endpoint_thread, keep_running_thread)
        });
    
        Ok(Self {
            keep_running,
            handle: Some(handle)
        })
    }

    pub fn stop(&mut self) {
        self.keep_running.store(false, Ordering::SeqCst);
        self.handle.take().expect("Mmm")
            .join().expect("Mmm");
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

    /// Thread loop reading data from the HGI80
    fn evohome_loop<R: BufRead>(reader: &mut R, endpoint: String, keep_running: Arc<AtomicBool>)
    {
        let mut line = String::new();

        while keep_running.load(Ordering::SeqCst) {
            match reader.read_line(&mut line) {
                Ok(_bytes) => match parse_packet(line.as_str()) {
                    Ok(packet) => {
                        // Packets with an RSSI higher than 80 usually have lots of bit errors
                        if packet.rssi < 80 { 
                            if let Some(Payload::ZoneTemp(zt)) = packet.payload {
                                Self::post_temperature_data(&endpoint, &zt.temperatures);

                                println!("Temperature {:?}", zt.temperatures);
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


trait PayloadFrom {
    type PayloadType;
    fn from_payload(data: &str) -> Result<Self::PayloadType>;
}

pub struct ZoneTemp {
    pub temperatures: HashMap<u8, f32>,
}

impl PayloadFrom for ZoneTemp {
    type PayloadType = Self;
    fn from_payload(data: &str) -> Result<Self::PayloadType> {
        //045  I --- 01:073979 --:------ 01:073979 30C9 024 0007070106CE0206C70307320405FB05070F06064D070789
        if (data.len() % 6) != 0 {
            bail!(
                "Zone temperature payload should be a multiple of 6 characters (payload {})",
                data
            );
        }

        let mut temperatures: HashMap<u8, f32> = HashMap::new();
        // It can be any number of id, temperature pairs
        for i in (0..data.len()).step_by(6) {
            let id = u8::from_str_radix(&data[i..i + 2], 16)
                .with_context(|| format!("Invalid zone ID in '{}' (i={})", data, i))?;
            let centi_degrees = i32::from_str_radix(&data[i + 2..i + 6], 16)
                .with_context(|| format!("Invalid zone temperature in '{}' (i={})", data, i))?;
            temperatures.insert(id, centi_degrees as f32 / 100.0);
        }

        Ok(ZoneTemp { temperatures })
    }
}

pub enum PacketType {
    Unknown,
    Information,
    Request,
    Response,
    Write,
}

#[derive(FromPrimitive, Debug)]
pub enum Command {
    SysInfo = 0x10e0,
    ZoneTemp = 0x30C9,
    ZoneName = 0x0004,
    ZoneHeatDemand = 0x3150, //Heat demand sent by an individual zone
    ZoneInfo = 0x000A,
    ZoneWindow = 0x12B0, //Open window/ventilation zone function
    SetPoint = 0x2309,
    SetpointOverride = 0x2349,
    DHWState = 0x1F41,
    DHWTemp = 0x1260,
    DHWSettings = 0x10A0, //DHW settings sent between controller and DHW sensor can also be requested by the gateway
    ControllerMode = 0x2E04,
    ControllerHeatDemand = 0x0008, //Heat demand sent by the controller for CH / DHW / Boiler  (F9/FA/FC)
    OpenThermBridge = 0x3220,      //OT Bridge Status messages
    OpenThermSetpoint = 0x22D9,    //OT Bridge Control Setpoint
    ActuatorState = 0x3EF0,
    ActuatorCheck = 0x3B00,
    Binding = 0x1FC9,
    ExternalSensor = 0x0002,
    DeviceInfo = 0x0418,
    BatteryInfo = 0x1060,
    Sync = 0x1F09,
    //0x10a0 //DHW settings sent between controller and DHW sensor can also be requested by the gateway <1:DevNo><2(uint16_t):SetPoint><1:Overrun?><2:Differential>
    //0x0005
    //0x0006
    //0x0404
    Unknown = 0xFFFF,
}

pub enum Payload {
    ZoneTemp(ZoneTemp),
}

pub struct Packet {
    pub rssi: u16,
    pub packet_type: PacketType,
    pub command: Command,
    pub payload: Option<Payload>,
}

impl Packet {
    fn new() -> Self {
        Self {
            rssi: 0,
            packet_type: PacketType::Unknown,
            command: Command::Unknown,
            payload: None,
        }
    }
}

fn parse_packet_type(data: &str) -> Result<PacketType> {
    match data {
        "RP" => Ok(PacketType::Response),
        "RQ" => Ok(PacketType::Request),
        "I" => Ok(PacketType::Information),
        "W" => Ok(PacketType::Write),
        _ => Err(format_err!("Unknown packet type {}", data)),
    }
}

fn parse_command(data: &str) -> Result<Command> {
    let cmd_id = u32::from_str_radix(data, 16).with_context(|| "Invalid command ID")?;
    let cmd = match num::FromPrimitive::from_u32(cmd_id) {
        Some(Command::SysInfo) => Command::SysInfo,
        Some(Command::ZoneTemp) => Command::ZoneTemp,
        Some(Command::ZoneName) => Command::ZoneName,
        Some(Command::ZoneHeatDemand) => Command::ZoneHeatDemand,
        Some(Command::ZoneInfo) => Command::ZoneInfo,
        Some(Command::ZoneWindow) => Command::ZoneWindow,
        Some(Command::SetPoint) => Command::SetPoint,
        Some(Command::SetpointOverride) => Command::SetpointOverride,
        Some(Command::DHWState) => Command::DHWState,
        Some(Command::DHWTemp) => Command::DHWTemp,
        Some(Command::DHWSettings) => Command::DHWSettings,
        Some(Command::ControllerMode) => Command::ControllerMode,
        Some(Command::ControllerHeatDemand) => Command::ControllerHeatDemand,
        Some(Command::OpenThermBridge) => Command::OpenThermBridge,
        Some(Command::OpenThermSetpoint) => Command::OpenThermSetpoint,
        Some(Command::ActuatorState) => Command::ActuatorState,
        Some(Command::ActuatorCheck) => Command::ActuatorCheck,
        Some(Command::Binding) => Command::Binding,
        Some(Command::ExternalSensor) => Command::ExternalSensor,
        Some(Command::DeviceInfo) => Command::DeviceInfo,
        Some(Command::BatteryInfo) => Command::BatteryInfo,
        Some(Command::Sync) => Command::Sync,
        _ => Command::Unknown,
    };

    if let Command::Unknown = cmd {
        bail!("Unknown command {}", data);
    }

    Ok(cmd)
}

fn parse_payload(command: &Command, data: &str) -> Result<Payload> {
    match command {
        Command::ZoneTemp => {
            let zt = ZoneTemp::from_payload(data)?;
            Ok(Payload::ZoneTemp(zt))
        }
        _ => Err(format_err!(
            "Don't know how to parse the payload for {:?}",
            command
        )),
    }
}

pub fn parse_packet(data: &str) -> Result<Packet> {
    const EXPECTED_COLUMNS: usize = 9;

    // Non ascii characters appear between the sentences, Filter these.
    let filtered: String = data.chars().filter(|c| !c.is_ascii_control()).collect();
    let columns: Vec<&str> = filtered.split_ascii_whitespace().collect();
    if columns.len() != EXPECTED_COLUMNS {
        bail!("Column count should be {} '{}'", EXPECTED_COLUMNS, filtered);
    }

    //Check payload size (2 chars per byte)
    let payload_chars = 2 * columns[7]
        .parse::<usize>()
        .with_context(|| "While parsing the payload size")?;
    if payload_chars != columns[8].len() {
        bail!(
            "Payload size does not match, expected {} chars, got {}",
            payload_chars,
            columns[8].len()
        );
    }

    let mut packet = Packet::new();
    packet.rssi = columns[0].parse::<u16>().with_context(|| {
        format!(
            "While parsing the rssi (column 0) '{}' {:?}",
            columns[0],
            columns[0].as_bytes()
        )
    })?;
    packet.packet_type = parse_packet_type(columns[1])?;
    packet.command = parse_command(columns[6])?;
    packet.payload = Some(parse_payload(&packet.command, columns[8])?);

    Ok(packet)
}

#[cfg(test)]
mod line_parsing_tests {
    use super::*;

    #[test]
    fn parse_too_few_colums() {
        assert!(parse_packet("").is_err());
        assert!(parse_packet("1 2 3 4 5 6 7 8 ").is_err());
    }

    #[test]
    fn parse_payload_length_mismatch() {
        // Note that payload refers to decoded bytes (two payload characters form a byte)
        // 1 char short
        assert!(
            parse_packet("063  I --- 04:143260 --:------ 04:143260 30C9 006 00070203081").is_err()
        );

        // 1 char oversize
        assert!(
            parse_packet("063  I --- 04:143260 --:------ 04:143260 30C9 006 000702030814A")
                .is_err()
        );
    }

    #[test]
    fn parse_zonetemp() {
        let packet =
            parse_packet("063  I --- 04:143260 --:------ 04:143260 30C9 006 000702030814").unwrap();
        if let Some(Payload::ZoneTemp(zt)) = packet.payload {
            assert_eq!(zt.temperatures.len(), 2);
            assert_eq!(zt.temperatures[&0u8], 17.94);
            assert_eq!(zt.temperatures[&3u8], 20.68);
        } else {
            assert!(false);
        }
    }

    #[test]
    fn invalid_characters_are_filtered_out() {
        // Non readable ASCII characters are output by the HGI80 quite often
        let packet = parse_packet(
            "\x11\x11095  I --- 04:112669 --:------ 04:112669 30C9 003 0006E1\x11\x11\n",
        )
        .unwrap();
        // The unwrap should do it, but test the RSSI, the parsing of which put me
        // on the trail of the bogus data
        assert_eq!(packet.rssi, 95);
    }
}

#[cfg(test)]
mod zone_temp_tests {
    use super::*;

    #[test]
    fn parse_zonetemp() {
        assert_eq!(
            ZoneTemp::from_payload("030702010814").unwrap().temperatures,
            HashMap::from([(3u8, 17.94), (1u8, 20.68),])
        );

        assert_eq!(
            ZoneTemp::from_payload("040702").unwrap().temperatures,
            HashMap::from([(4u8, 17.94),])
        );

        assert_eq!(
            ZoneTemp::from_payload("").unwrap().temperatures,
            HashMap::new()
        );
    }

    #[test]
    fn parse_zonetemp_errors() {
        // Not multiples of 6 chars
        assert!(ZoneTemp::from_payload("01").is_err());
        assert!(ZoneTemp::from_payload("01020").is_err());

        // Non-hex characters
        assert!(ZoneTemp::from_payload("1234X6").is_err());
        assert!(ZoneTemp::from_payload("1X3456").is_err());
    }
}
