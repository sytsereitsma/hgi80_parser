use clap::Parser;
use hgi80_decoder::hgi80::HGI80;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Full URL string of endpoint to send the data to (e.g. https://www.foo.bar/baz)
    #[arg(short, long)]
    endpoint: String,

    /// Name of USB device to connect to (.g. COM7, or /dev/ttyUSB0)
    #[arg(short, long)]
    usb: String,
}

fn main() {
    let args = Args::parse();

    let evohome = HGI80::new(&args.usb, &args.endpoint);
}
