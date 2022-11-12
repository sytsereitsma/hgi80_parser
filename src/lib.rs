extern crate num;
#[macro_use]
extern crate num_derive;
use anyhow::{bail, format_err, Context, Result};

trait PayloadFrom {
    type PayloadType;
    fn from_payload(data: &str) -> Result<Self::PayloadType>;
}

pub struct ZoneTemp {
    pub temperature: f32,
}

impl PayloadFrom for ZoneTemp {
    type PayloadType = Self;
    fn from_payload(data: &str) -> Result<Self::PayloadType> {
        let centi_degrees =
            i32::from_str_radix(&data[0..6], 16).with_context(|| "Invalid zone temperature")?;
        Ok(ZoneTemp {
            temperature: centi_degrees as f32 / 100.0,
        })
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
    pub flags: u16,
    pub packet_type: PacketType,
    pub command: Command,
    pub id: [String; 3],
    pub payload: Option<Payload>,
}

impl Packet {
    fn new() -> Self {
        Self {
            flags: 0,
            packet_type: PacketType::Unknown,
            command: Command::Unknown,
            id: Default::default(), 
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
        bail!("Unknow command {}", data);
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

    let columns: Vec<&str> = data.split_ascii_whitespace().collect();
    if columns.len() != EXPECTED_COLUMNS {
        bail!("Column count should be {}", EXPECTED_COLUMNS);
    }

    let mut packet = Packet::new();
    packet.flags = u16::from_str_radix(columns[0], 16)
        .with_context(|| "While pasring the flags (column 0)")?;
    packet.packet_type = parse_packet_type(columns[1])?;
    packet.command = parse_command(columns[6])?;
    packet.id[0] = String::from(columns[3]);
    packet.id[1] = String::from(columns[4]);
    packet.id[2] = String::from(columns[5]);
    packet.payload = Some(parse_payload(&packet.command, columns[8])?);
    Ok(packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zonetemp() {
        let packet =
            parse_packet("063  I --- 04:143260 --:------ 04:143260 30C9 003 000702").unwrap();
        if let Some(Payload::ZoneTemp(zt)) = packet.payload {
            assert_eq!(zt.temperature, 17.94);
            assert_eq!(packet.id[0], "04:143260");
            assert_eq!(packet.id[1], "--:------");
            assert_eq!(packet.id[2], "04:143260");
        } else {
            assert!(false);
        }
    }
}
