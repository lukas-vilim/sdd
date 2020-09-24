mod lib;

use lib::dae;
// use structopt::StructOpt;

// #[derive(StructOpt)]
// struct Cli {
// 	/// Target Ip and port.
// 	#[structopt(short = "a", long = "address")]
// 	addr: String,
// 	/// Output file path.
// 	#[structopt(parse(from_os_str), short = "o", long = "output")]
// 	output: std::path::PathBuf,
// }

fn main() {
	let protocol = match dae::Protocol::new(String::from("resources/test.db")) {
		Ok(p) => p,
		Err(e) => {
			println!("{}", e);
			return;
		}
	};

	let mut daemon = dae::Daemon { proto: protocol };

	match daemon.run(&String::from("127.0.0.1:2001")) {
		Ok(()) => {}
		Err(e) => {
			println!("{}", e);
		}
	};
}
