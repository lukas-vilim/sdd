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
		fn sql_from_raw<R: Read>(
			&mut self,
			reader: &mut BufReader<R>,
		) -> Result<&dyn rusqlite::ToSql, std::io::Error> {
			match &mut self.data_type {
				FieldType::Int(data) => {
					let mut bytes = [0; 4];
					reader.read_exact(&mut bytes)?;

					*data = u32::from_le_bytes(bytes);
					Ok(data)
				}
				FieldType::Float(data) => {
					let mut bytes = [0; 4];
					reader.read_exact(&mut bytes)?;

					*data = f32::from_le_bytes(bytes).into();
					Ok(data)
				}
				FieldType::Bool(data) => {
					let mut bytes = [0; 1];
					reader.read_exact(&mut bytes)?;

					*data = bytes[0] > 0;
					Ok(data)
				}
				FieldType::Str(data) => {
					let mut bytes = [0; 4];
					reader.read_exact(&mut bytes)?;

					*data = u32::from_le_bytes(bytes);
					Ok(data)
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
				Err(_) => return Result::Err("Connection error"),
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
		fn read_descriptor<R: Read>(
			reader: &mut BufReader<R>,
		) -> Result<(EntryDescriptor, u32), std::io::Error> {
			let mut msg_id_bytes = [0; 4];
			let mut msg_num_fields_bytes = [0; 1];
			reader.read_exact(&mut msg_id_bytes)?;
			reader.read_exact(&mut msg_num_fields_bytes)?;

			let msg_id = u32::from_le_bytes(msg_id_bytes);
			let msg_num_fields = msg_num_fields_bytes[0] as usize;

			let mut desc = EntryDescriptor::make();
			desc.num_fields = msg_num_fields_bytes[0];

			for i in 0..msg_num_fields {
				let mut data_type_bytes = [0; 1];
				let mut name_bytes = [0; 4];

				reader.read_exact(&mut data_type_bytes)?;
				reader.read_exact(&mut name_bytes)?;

				let data_type = FieldType::from(data_type_bytes[0]);
				let name = u32::from_le_bytes(name_bytes);
				let field = Field { data_type, name };

				desc.fields[i as usize] = Option::Some(field);
			}

			Result::Ok((desc, msg_id))
		}

		fn find_descriptor<'a, 'b, R: Read>(
			reader: &'a mut BufReader<R>,
			register: &'b mut Vec<EntryDescriptor>,
		) -> Result<&'b mut EntryDescriptor, ParsingError> {
			let mut uid_bytes = [0; 4];
			match reader.read_exact(&mut uid_bytes) {
				Ok(_) => {}
				Err(_) => return Err(ParsingError::Space),
			};

			let uid = u32::from_le_bytes(uid_bytes);
			if register.len() < uid as usize {
				return Err(ParsingError::Fatal(
					"Uid not found among the descriptors",
				));
			}

			Result::Ok(&mut register[uid as usize])
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
			let stream = TcpStream::connect(addr)
				.expect("Could not connect to the address.");
			let mut reader = BufReader::new(stream);

			enum State {
				HeaderParsing,
				DescParsing,
				EntryParsing,
				StringParsing,
			};

			let mut state = State::HeaderParsing;

			// Read protocol messages until shutdown.
			loop {
				state = match state {
					State::HeaderParsing => {
						let mut proto_bytes: [u8; 4] = [0; 4];
						match reader.read_exact(&mut proto_bytes) {
							Ok(_) => {}
							Err(_) => {
								continue;
							}
						};

						match u32::from_le_bytes(proto_bytes) {
							PROTOCOL => {}
							_ => continue,
						}

						let mut type_bytes: [u8; 1] = [0];
						match reader.read_exact(&mut type_bytes) {
							Ok(_) => {}
							Err(_) => {
								continue;
							}
						}

						let new_state = match type_bytes[0].try_into().unwrap()
						{
							MsgType::Desc => State::DescParsing,
							MsgType::Entry => State::EntryParsing,
							MsgType::Str => State::StringParsing,
							MsgType::Invalid => State::HeaderParsing,
						};

						new_state
					}
					State::DescParsing => {
						let (mut desc, uid) =
							match Daemon::read_descriptor(&mut reader) {
								Ok((desc, uid)) => (desc, uid),
								Err(e) => {
									println!("Failure during read_descriptor!");
									println!("{}", e);

									break;
								}
							};

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
						let desc = match Daemon::find_descriptor(
							&mut reader,
							&mut self.proto.descriptors,
						) {
							Ok(desc) => desc,
							Err(ParsingError::Space) => {
								break;
							}
							Err(ParsingError::Fatal(msg)) => {
								return Result::Err(msg);
							}
						};

						let mut params =
							Vec::<&dyn rusqlite::ToSql>::with_capacity(
								desc.num_fields as usize,
							);

						for field in &mut desc.fields {
							match field {
								Some(val) => {
									let to_sql =
										match val.sql_from_raw(&mut reader) {
											Ok(val) => val,
											Err(e) => {
												println!("Error during the sql_from_raw!");
												println!("{}", e);

												continue;
											}
										};

									params.push(to_sql);
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
						let mut uid_bytes = [0;4];
						match reader.read_exact(&mut uid_bytes) {
							Ok(_) => {}
							Err(_) => { continue; }
						};

						let uid = u32::from_le_bytes(uid_bytes);
						if uid as usize != self.proto.strings.len() {
							// error string ids broken.
							println!("String uid does not match!");
							continue;
						}

						let mut size_bytes = [0;4];
						match reader.read_exact(&mut size_bytes) {
							Ok(_) => {}
							Err(_) => { continue; }
						};

						let size = u32::from_le_bytes(size_bytes);
						let mut string_bytes = Vec::<u8>::with_capacity(size as usize);
						match reader.read_exact(&mut string_bytes[..])
						{
							Ok(_) => {}
							Err(_) => { continue; }
						};

						let string = match String::from_utf8(string_bytes) {
							Ok(s) => s,
							Err(e) => { println!("{}", e); continue; }
						};

						self.proto.strings.push(string);

						State::HeaderParsing
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
