use hgi80_decoder::{parse_packet, Payload};

fn main() {
	let packet = parse_packet("063  I --- 04:143260 --:------ 04:143260 30C9 003 000702").unwrap();
    if let Some(Payload::ZoneTemp(zt)) = packet.payload {
		println!("Temperature {} = {}", &packet.id[0], zt.temperature);
	}
}