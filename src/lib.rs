pub mod dae {
	use rusqlite;
	use std::convert::TryInto;
	use std::io::BufReader;
	use std::io::Read;
	use std::net::TcpStream;

	const PROTOCOL: u32 = 0xFEEDBEEF;

	#[derive(Debug, Copy, Clone)]
	enum FieldType {
		Int(u32),
		Float(f64),
		Bool(bool),
		Str(u32),
	}

	#[derive(Copy, Clone)]
	struct Field {
		data_type: FieldType,
		name: u32,
	}

	impl PartialEq for FieldType {
		fn eq(&self, other: &Self) -> bool {
			match (self, other) {
				(FieldType::Int(..), FieldType::Int(..)) => true,
				(FieldType::Float(..), FieldType::Float(..)) => true,
				(FieldType::Bool(..), FieldType::Bool(..)) => true,
				(FieldType::Str(..), FieldType::Str(..)) => true,
				_ => false,
			}
		}
	}

	impl From<u8> for FieldType {
		fn from(t: u8) -> Self {
			match t {
				1 => FieldType::Int(0),
				2 => FieldType::Float(0.0),
				3 => FieldType::Bool(false),
				4 => FieldType::Str(0),
				_ => panic!(),
			}
		}
	}

	impl ToString for FieldType {
		fn to_string(&self) -> String {
			match self {
				FieldType::Int(..) => String::from("INTEGER"),
				FieldType::Float(..) => String::from("REAL"),
				FieldType::Bool(..) => String::from("INTEGER"),
				FieldType::Str(..) => String::from("TEXT"),
			}
		}
	}

	impl Field {
		fn sql_from_raw(
			&mut self,
			raw: &[u8],
		) -> (&dyn rusqlite::ToSql, usize) {
			match &mut self.data_type {
				FieldType::Int(data) => {
					*data = u32::from_le_bytes(raw[0..4].try_into().unwrap());
					(data, 4)
				}
				FieldType::Float(data) => {
					*data = f32::from_le_bytes(raw[0..4].try_into().unwrap())
						.into();
					(data, 4)
				}
				FieldType::Bool(data) => {
					*data = raw[0] > 0;
					(data, 1)
				}
				FieldType::Str(data) => {
					*data = u32::from_le_bytes(raw[0..4].try_into().unwrap());
					(data, 4)
				}
			}
		}
	}

	#[derive(Clone)]
	struct EntryDescriptor {
		sql_cmd: String,
		name: u32,
		num_fields: u8,
		fields: [Option<Field>; 32],
	}

	impl EntryDescriptor {
		pub fn make() -> EntryDescriptor {
			EntryDescriptor {
				sql_cmd: String::from("INSERT INTO "),
				name: 0,
				num_fields: 0,
				fields: [Option::None; 32],
			}
		}

		pub fn compile(&mut self, strings: &Vec<String>) {
			let name = &strings.get(self.name as usize).unwrap();
			self.sql_cmd.push_str(name);
			self.sql_cmd.push_str(" (");

			for i in 0..(self.num_fields as usize) {
				let field = &self.fields[i].unwrap();

				let name = &strings.get(field.name as usize).unwrap();
				self.sql_cmd.push_str(name);
				self.sql_cmd.push_str(" ");
				self.sql_cmd.push_str(&field.data_type.to_string());

				if i < self.num_fields as usize - 1 {
					self.sql_cmd.push_str(", ");
				} else {
					self.sql_cmd.push_str(")");
				}
			}
		}

		pub fn make_create_cmd(&self, strings: &Vec<String>) -> String {
			let mut cmd = String::from("CREATE TABLE ");
			cmd.push_str(&strings[self.name as usize]);
			cmd.push_str(" (");

			fn push_param(
				cmd: &mut String,
				field: &Field,
				strings: &Vec<String>,
			) {
				cmd.push_str(&strings[field.name as usize]);
				cmd.push_str(" ");
				cmd.push_str(&field.data_type.to_string());
			}

			let num_fields = self.num_fields as usize;
			for i in 0..num_fields - 1 {
				let field = &self.fields[i].unwrap();
				push_param(&mut cmd, field, strings);
				cmd.push_str(", ");
			}

			let last_field = &self.fields[num_fields - 1].unwrap();
			push_param(&mut cmd, last_field, strings);
			cmd.push_str(")");

			return cmd;
		}
	}

	pub struct Protocol {
		con: rusqlite::Connection,
		descriptors: Vec<EntryDescriptor>,
		strings: Vec<String>,
	}

	impl Protocol {
		pub fn new(db_path: String) -> Result<Protocol, &'static str> {
			let connection = match rusqlite::Connection::open(db_path) {
				Ok(c) => c,
				Err(e) => return Result::Err("Connection error"),
			};

			let proto = Protocol {
				con: connection,
				descriptors: vec![],
				strings: vec![],
			};

			Result::Ok(proto)
		}
	}

	enum MsgType {
		Invalid = 0,
		Str = 1,
		Entry = 2,
		Desc = 3,
	}

	enum ParsingError {
		Space,
		Fatal(&'static str),
	}

	impl From<u8> for MsgType {
		fn from(t: u8) -> Self {
			match t {
				1 => MsgType::Desc,
				2 => MsgType::Entry,
				3 => MsgType::Str,
				_ => MsgType::Invalid,
			}
		}
	}

	pub struct Daemon {
		pub proto: Protocol,
	}

	impl Daemon {
		fn read_descriptor(
			buf: &[u8],
		) -> Result<(EntryDescriptor, u32, usize), ParsingError> {
			// Read type 1B, id 4B, dependency 1B and num fields 1B.
			if buf.len() < 5 {
				return Result::Err(ParsingError::Space);
			};

			let msg_id = u32::from_le_bytes(buf[0..4].try_into().unwrap());
			let msg_num_fields = buf[4];

			let mut desc = EntryDescriptor::make();
			desc.num_fields = msg_num_fields;

			let expected_size: usize = 5 + msg_num_fields as usize * 5;
			if buf.len() < expected_size {
				return Result::Err(ParsingError::Space);
			}

			for i in 0..msg_num_fields {
				let byte_idx: usize = 5 + i as usize * (1 + 4);
				let data_type = FieldType::from(buf[byte_idx]);
				let name = u32::from_le_bytes(
					buf[byte_idx + 1..byte_idx + 5].try_into().unwrap(),
				);
				let field = Field { data_type, name };

				desc.fields[i as usize] = Option::Some(field);
			}

			Result::Ok((desc, msg_id, expected_size))
		}

		fn find_descriptor<'a, 'b>(
			buf: &'a [u8],
			register: &'b mut Vec<EntryDescriptor>,
		) -> Result<(&'b mut EntryDescriptor, usize), ParsingError> {
			if buf.len() < 4 {
				return Result::Err(ParsingError::Space);
			}

			let uid = u32::from_le_bytes(buf[0..4].try_into().unwrap());
			if register.len() < uid as usize {
				return Result::Err(ParsingError::Fatal(
					"Uid not found among the descriptors",
				));
			}

			Result::Ok((&mut register[uid as usize], 4))
		}

		fn register_descriptor<'a>(
			desc: EntryDescriptor,
			uid: u32,
			register: &'a mut Vec<EntryDescriptor>,
		) -> Result<(), &'static str> {
			if uid as usize != register.len() {
				return Result::Err("Unexpected UID");
			}

			register.push(desc);
			Result::Ok(())
		}

		pub fn run(&mut self, addr: &String) -> Result<(), &'static str> {
			let mut stream = TcpStream::connect(addr)
				.expect("Could not connect to the address.");

			// let mut reader = BufReader::new(stream);
			let mut buf: [u8; 256] = [0; 256];
			let mut buf_ptr: usize = 0;
			let mut buf_len: usize = 0;

			enum State {
				HeaderParsing,
				DescParsing,
				EntryParsing,
				StringParsing,
			};

			let mut state = State::HeaderParsing;

			// Read protocol messages.
			loop {
				// Shift the rest of the buffer back to the begining.
				if buf_ptr > 0 && buf_len > 0 {
					let mut ptr = 0;
					for n in buf_ptr..buf_len {
						buf[ptr] = buf[n];
						ptr = ptr + 1;
					}
				}

				match stream.read(&mut buf[buf_ptr..256]) {
					Ok(size) => {
						buf_len = size + buf_ptr;
					}
					Err(_e) => {
						print!("{}", _e);
						break;
					}
				};

				// Loop until theres not enough data in the buffer.
				loop {
					state = match state {
						State::HeaderParsing => {
							if buf_len - buf_ptr < 5 {
								break;
							}

							let mut proto_bytes: [u8; 4] = [0; 4];
							proto_bytes
								.copy_from_slice(&buf[buf_ptr..buf_ptr + 4]);

							let proto = u32::from_le_bytes(proto_bytes);
							if proto != PROTOCOL {
								buf_ptr += 4;
								continue;
							}

							let new_state =
								match buf[buf_ptr + 4].try_into().unwrap() {
									MsgType::Desc => State::DescParsing,
									MsgType::Entry => State::EntryParsing,
									MsgType::Str => State::StringParsing,
									MsgType::Invalid => State::HeaderParsing,
								};

							buf_ptr += 5;
							new_state
						}
						State::DescParsing => {
							let (mut desc, uid, read) =
								match Daemon::read_descriptor(
									&buf[buf_ptr..buf_len],
								) {
									Ok((desc, uid, read)) => (desc, uid, read),
									Err(ParsingError::Space) => {
										break;
									}
									Err(ParsingError::Fatal(msg)) => {
										return Result::Err(msg);
									}
								};

							buf_ptr += read;

							desc.compile(&self.proto.strings);

							let create_cmd =
								desc.make_create_cmd(&self.proto.strings);

							Daemon::register_descriptor(
								desc,
								uid,
								&mut self.proto.descriptors,
							)?;

							self.proto
								.con
								.execute(&create_cmd, rusqlite::NO_PARAMS)
								.expect("SQL creation query failed");

							State::HeaderParsing
						}
						State::EntryParsing => {
							let (desc, read) = match Daemon::find_descriptor(
								&buf[buf_ptr..buf_len],
								&mut self.proto.descriptors,
							) {
								Ok((desc, read)) => (desc, read),
								Err(ParsingError::Space) => {
									break;
								}
								Err(ParsingError::Fatal(msg)) => {
									return Result::Err(msg);
								}
							};

							buf_ptr += read;

							let mut params =
								Vec::<&dyn rusqlite::ToSql>::with_capacity(
									desc.num_fields as usize,
								);

							for field in &mut desc.fields {
								match field {
									Some(val) => {
										let (to_sql, size) = val.sql_from_raw(
											&buf[buf_ptr..buf_len],
										);

										params.push(to_sql);
										buf_ptr += size;
									}
									_ => {
										break;
									}
								}
							}

							let con = &self.proto.con;
							let cmd = &desc.sql_cmd;

							con.execute(cmd, params).expect("SQL Query failed");

							State::HeaderParsing
						}
						State::StringParsing => {
							// TODO:
							State::HeaderParsing
						}
					}
				}
			}

			Result::Ok(())
		}
	}

	// pub struct LocalServer {}

	// impl LocalServer {
	// 	pub fn new() -> LocalServer {
	// 		LocalServer {}
	// 	}

	// 	pub fn client_loop(mut _stream: TcpStream) {}

	// 	pub fn run(&self, port: u16) {
	// 		let mut addr = String::from("0.0.0.0:");
	// 		addr.push_str(&port.to_string());

	// 		let listener = TcpListener::bind(addr.as_str()).unwrap();

	// 		loop {
	// 			for stream in listener.incoming() {
	// 				match stream {
	// 					Ok(stream) => {
	// 						thread::spawn(move || {
	// 							LocalServer::client_loop(stream)
	// 						});
	// 					}
	// 					Err(e) => {
	// 						println!("Error accepting the stream: {}", e);
	// 					}
	// 				}
	// 			}
	// 		}

	// 		// 			drop(listener);
	// 	}
	// }

	#[cfg(test)]
	mod tests {
		use super::*;

		#[test]
		fn read_proto() {
			let data: [u8; 15] = [
				0x6, 0x0, 0x0, 0x0, // id
				0x2, // num_fields
				0x1, // field type
				0x7, 0x0, 0x0, 0x0, // field name
				0x2, // field type
				0x8, 0x0, 0x0, 0x0, // field name
			];

			match Daemon::read_descriptor(&data) {
				Ok((desc, id, _read)) => {
					assert_eq!(id, 6);
					assert_eq!(desc.num_fields, 2);

					fn match_field(
						field: Option<Field>,
						field_type: u8,
						name: u32,
					) {
						match field {
							Some(x) => {
								assert_eq!(
									x.data_type,
									FieldType::from(field_type)
								);
								assert_eq!(x.name, name);
							}
							_ => panic!(),
						};
					}

					match_field(desc.fields[0], 1, 7);
					match_field(desc.fields[1], 2, 8);
				}
				Err(ParsingError::Fatal(msg)) => {
					println!("{}", msg);
					panic!()
				}
				_ => panic!(),
			};
		}
	}
}
