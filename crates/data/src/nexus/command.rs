use anyhow::{Context, Result, bail, ensure};

use std::io::BufRead;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
	source: String,
	line: usize,
	column: usize,
}

impl Command {
	pub fn source(&self) -> &str {
		&self.source
	}

	pub fn line(&self) -> usize {
		self.line
	}

	pub fn column(&self) -> usize {
		self.column
	}
}

#[derive(Debug)]
pub struct CommandReader<R> {
	reader: R,
	line_buffer: String,
	offset: usize,
	line: usize,
	column: usize,
	finished: bool,
}

impl<R: BufRead> CommandReader<R> {
	pub fn new(mut reader: R) -> Result<Self> {
		let mut line_buffer = String::new();
		reader.read_line(&mut line_buffer)?;
		let first_line = line_buffer.as_bytes();
		let mut offset = if first_line.starts_with(&[0xef, 0xbb, 0xbf])
		{
			3
		} else {
			0
		};
		while first_line
			.get(offset)
			.is_some_and(u8::is_ascii_whitespace)
		{
			offset += 1;
		}
		let end = offset + b"#NEXUS".len();
		ensure!(
			first_line
				.get(offset..end)
				.is_some_and(|value| value
					.eq_ignore_ascii_case(b"#NEXUS")),
			"Expected '#NEXUS' at the start of the file"
		);
		ensure!(
			first_line.get(end).is_none_or(|byte| {
				byte.is_ascii_whitespace() || *byte == b'['
			}),
			"Expected whitespace after '#NEXUS'"
		);

		Ok(Self {
			reader,
			line_buffer,
			offset: end,
			line: 1,
			column: end + 1,
			finished: false,
		})
	}

	pub fn next_command(&mut self) -> Result<Option<Command>> {
		if self.finished {
			return Ok(None);
		}

		let mut source = Vec::new();
		let mut start: Option<(usize, usize)> = None;
		let mut comment_depth = 0_u32;
		let mut quote = None;
		let mut has_content = false;

		'input: loop {
			if self.offset == self.line_buffer.len() {
				self.line_buffer.clear();
				self.offset = 0;
			}
			if self.line_buffer.is_empty()
				&& self.reader
					.read_line(&mut self.line_buffer)? == 0
			{
				self.finished = true;
				ensure!(
					comment_depth == 0,
					"Unterminated NEXUS comment starting before line {}",
					self.line
				);
				ensure!(
					quote.is_none(),
					"Unterminated quoted NEXUS token starting before line {}",
					self.line
				);
				if !has_content {
					return Ok(None);
				}
				bail!(
					"Unterminated NEXUS command starting at line {}, column {}",
					start.map_or(self.line, |position| {
						position.0
					}),
					start.map_or(self.column, |position| {
						position.1
					})
				);
			}

			while self.offset < self.line_buffer.len() {
				let byte = self.line_buffer.as_bytes()
					[self.offset];
				if start.is_none() {
					if byte.is_ascii_whitespace() {
						self.advance_byte(byte);
						self.offset += 1;
						continue;
					}
					start = Some((self.line, self.column));
				}
				source.push(byte);
				self.advance_byte(byte);

				if comment_depth > 0 {
					match byte {
						b'[' => comment_depth += 1,
						b']' => comment_depth -= 1,
						_ => {}
					}
					self.offset += 1;
					continue;
				}

				if let Some(delimiter) = quote {
					if byte == delimiter {
						if self.line_buffer
							.as_bytes()
							.get(self.offset + 1) == Some(
							&delimiter,
						) {
							self.offset += 1;
							source.push(delimiter);
							self.advance_byte(
								delimiter,
							);
						} else {
							quote = None;
						}
					}
					self.offset += 1;
					continue;
				}
				if !byte.is_ascii_whitespace()
					&& !matches!(byte, b'[' | b';')
				{
					has_content = true;
				}

				match byte {
					b'[' => comment_depth = 1,
					b']' => bail!(
						"Unexpected ']' at line {}, column {}",
						self.line,
						self.column.saturating_sub(1)
					),
					b'\'' | b'"' => quote = Some(byte),
					b';' => {
						self.offset += 1;
						if !has_content {
							source.clear();
							start = None;
							continue 'input;
						}
						let (line, column) =
							start.unwrap();
						let source = String::from_utf8(
							source,
						)
						.context(
							"NEXUS input is not valid UTF-8",
						)?;
						return Ok(Some(Command {
							source,
							line,
							column,
						}));
					}
					_ => {}
				}
				self.offset += 1;
			}
		}
	}

	fn advance_byte(&mut self, byte: u8) {
		if byte == b'\n' {
			self.line += 1;
			self.column = 1;
		} else {
			self.column += 1;
		}
	}
}

impl<R: BufRead> Iterator for CommandReader<R> {
	type Item = Result<Command>;

	fn next(&mut self) -> Option<Self::Item> {
		match self.next_command() {
			Ok(Some(command)) => Some(Ok(command)),
			Ok(None) => None,
			Err(error) => {
				self.finished = true;
				Some(Err(error))
			}
		}
	}
}
