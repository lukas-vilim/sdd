pub mod dae {
	use std::convert::TryInto;
	use std::io::Read;
	use std::net::{TcpListener, TcpStream};
	use std::string;
	use std::thread;

	#[derive(Copy, Clone)]
	enum FieldType {
		Invalid = 0,
		Int = 1,
		Float = 2,
		Bool = 3,
		Name = 4,
	}

	impl From<u8> for FieldType {
		fn from(i: u8) -> Self {
			match i {
				1 => FieldType::Int,
				2 => FieldType::Float,
				3 => FieldType::Bool,
				4 => FieldType::Name,
				_ => FieldType::Invalid,
			}
		}
	}

	#[derive(Copy, Clone)]
	struct MsgType {
		num_fields: u8,
		fields: [Option<FieldType>; 32],
	}

	impl MsgType {
		pub fn make() -> MsgType {
			MsgType {
				num_fields: 0,
				fields: [Option::None; 32],
			}
		}
	}

	struct Protocol {
		sizes: [Option<u8>; 256],
		types: [Option<MsgType>; 256],
		dependencies: [Option<u8>; 256],
	}

	impl Protocol {
		pub fn make() -> Protocol {
			Protocol {
				sizes: [Option::None; 256],
				types: [Option::None; 256],
				dependencies: [Option::None; 256],
			}
		}
	}

	enum ProtoMsgType {
		Invalid = 0,
		Proto = 1,
		PritoEnd = 2,
		Name = 3,
	}

	struct Msg {
		type_id: u8,
		id: u32,
		data: Vec<u8>,
	}

	pub struct Daemon {
		proto: Protocol,
		name_map: Vec<String>,
	}

	impl Daemon {
		pub fn new() -> Daemon {
			Daemon {
				proto: Protocol::make(),
				name_map: vec![],
			}
		}

		// MsgType, id, dependency.
		fn read_proto<TStream: std::io::Read>(
			stream: &mut TStream,
		) -> Result<Option<(MsgType, u8, u32)>, &'static str> {
			let mut data: [u8; 256] = [0; 256];

			// Read type 1B, id 4B, dependency 1B and num fields 1B.
			match stream.read_exact(&mut data[0..7]) {
				Ok(()) => {}
				Err(_e) => return Result::Err("Could not read header bytes."),
			};

			let msg_type: usize = data[0].into();
			if msg_type != ProtoMsgType::Proto as usize {
				return Result::Ok(Option::None);
			};

			let msg_id = data[1];
			let msg_dependency: u32 = u32::from_be_bytes(data[2..6].try_into().unwrap());
			let msg_num_fields = data[6] as usize;

			match stream.read_exact(&mut data[7..msg_num_fields + 7]) {
				Ok(()) => {}
				Err(_e) => return Result::Err("Could not read fields bytes."),
			};

			let mut msg_type = MsgType::make();
			msg_type.num_fields = msg_num_fields as u8;

			for i in 0..msg_num_fields {
				msg_type.fields[i] = Option::Some(FieldType::from(data[7 + i]));
			}

			Result::Ok(Option::Some((msg_type, msg_id, msg_dependency)))
		}

		pub fn run(&mut self, addr: &String) {
			let mut stream = TcpStream::connect(addr).expect("Could not connect to the address.");

			let mut data: [u8; 256] = [0; 256];

			// Read protocol messages.
			loop {
				let res = match Daemon::read_proto(&mut stream) {
					Ok(o) => o,
					Err(_e) => {
						break;
					}
				};

				// Read type 1B, id 4B, dependency 1B and num fields 1B.
				match stream.read_exact(&mut data[0..7]) {
					Ok(()) => {}
					Err(_e) => {
						break;
					}
				}

				let msg_type: usize = data[0].into();
				if msg_type != ProtoMsgType::Proto as usize {
					break;
				}

				let msg_id: usize = data[1].into();
				match self.proto.types[msg_id] {
					None => {
						break;
					}
					Some(_i) => {}
				}

				let msg_dependency: u32 = u32::from_be_bytes(data[2..6].try_into().unwrap());
				match self.proto.types[msg_dependency as usize] {
					None => {
						break;
					}
					Some(_i) => {}
				}

				let msg_num_fields: usize = data[6] as usize;

				match stream.read_exact(&mut data[7..msg_num_fields + 7]) {
					Ok(()) => {}
					Err(_e) => {
						break;
					}
				}

				self.proto.types[msg_id] = Option::Some(MsgType::make());

				let msg_type = &mut self.proto.types[msg_id].unwrap();
				for i in 0..msg_num_fields {
					msg_type.fields[i] = Option::Some(FieldType::from(data[7 + i]));
				}
			}

			// Init protocol structures.

			// Read messages.
		}
	}

	pub struct LocalServer {}

	impl LocalServer {
		pub fn new() -> LocalServer {
			LocalServer {}
		}

		pub fn client_loop(mut stream: TcpStream) {}

		pub fn run(&self, port: u16) {
			let mut addr = String::from("0.0.0.0:");
			addr.push_str(&port.to_string());

			let listener = TcpListener::bind(addr.as_str()).unwrap();

			let mut run = true;
			loop {
				for stream in listener.incoming() {
					match stream {
						Ok(stream) => {
							thread::spawn(move || LocalServer::client_loop(stream));
						}
						Err(e) => {
							println!("Error accepting the stream: {}", e);
						}
					}
				}
			}

			drop(listener);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn read_proto() {
		// todo
	}
}
