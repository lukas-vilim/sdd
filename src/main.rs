mod lib;
use lib::dae;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
	/// Target Ip and port.
	#[structopt(short = "a", long = "address")]
	addr: String,
	/// Output file path.
	#[structopt(parse(from_os_str), short = "o", long = "output")]
	output: std::path::PathBuf,
}

fn main() {
	let mut daemon = dae::Daemon::new();
	daemon.run(&String::from("127.0.0.1:2001"));
}
