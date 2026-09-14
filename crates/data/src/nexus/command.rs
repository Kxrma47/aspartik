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
	pending: Vec<u8>,
	line: usize,
	column: usize,
	finished: bool,
}

impl<R: BufRead> CommandReader<R> {
	pub fn new(mut reader: R) -> Result<Self> {
		let mut first_line = Vec::new();
		reader.read_until(b'\n', &mut first_line)?;
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

		let mut result = Self {
			reader,
			pending: first_line[end..].to_vec(),
			line: 1,
			column: 1,
			finished: false,
		};
		result.advance_position(&first_line[..end]);
		Ok(result)
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
			let mut chunk = if self.pending.is_empty() {
				let mut chunk = Vec::new();
				self.reader.read_until(b';', &mut chunk)?;
				chunk
			} else {
				std::mem::take(&mut self.pending)
			};

			if chunk.is_empty() {
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

			let mut index = 0;
			while index < chunk.len() {
				let byte = chunk[index];
				if start.is_none() {
					if byte.is_ascii_whitespace() {
						self.advance_byte(byte);
						index += 1;
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
					index += 1;
					continue;
				}

				if let Some(delimiter) = quote {
					if byte == delimiter {
						if chunk.get(index + 1)
							== Some(&delimiter)
						{
							index += 1;
							source.push(delimiter);
							self.advance_byte(
								delimiter,
							);
						} else {
							quote = None;
						}
					}
					index += 1;
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
						self.pending = chunk
							.split_off(index + 1);
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
				index += 1;
			}
		}
	}

	fn advance_position(&mut self, bytes: &[u8]) {
		for &byte in bytes {
			self.advance_byte(byte);
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

#[cfg(test)]
mod tests {
	use anyhow::Result;

	use std::io::Cursor;

	use super::CommandReader;

	fn commands(input: &str) -> Result<Vec<(String, usize, usize)>> {
		CommandReader::new(Cursor::new(input))?
			.map(|command| {
				let command = command?;
				Ok((
					command.source().to_owned(),
					command.line(),
					command.column(),
				))
			})
			.collect()
	}

	#[test]
	fn reads_commands_and_locations() -> Result<()> {
		assert_eq!(
			commands(
				"#nexus BEGIN TREES;\n  TREE one = (A,B); END;"
			)?,
			[
				("BEGIN TREES;".to_owned(), 1, 8),
				("TREE one = (A,B);".to_owned(), 2, 3),
				("END;".to_owned(), 2, 21),
			]
		);
		Ok(())
	}

	#[test]
	fn preserves_nested_comments_and_quoted_semicolons() -> Result<()> {
		let input = "#NEXUS\n[outer; [inner;] done] BEGIN TREES;\nTREE 'a;''b' = ('x;y',A[&note='z;']);\nEND;";
		let commands = commands(input)?;
		assert_eq!(commands.len(), 3);
		assert_eq!(
			commands[0].0,
			"[outer; [inner;] done] BEGIN TREES;"
		);
		assert_eq!(
			commands[1].0,
			"TREE 'a;''b' = ('x;y',A[&note='z;']);"
		);
		assert_eq!(commands[2].0, "END;");
		Ok(())
	}

	#[test]
	fn accepts_bom_unicode_and_empty_files_after_header() -> Result<()> {
		assert!(commands("\u{feff}#NEXUS\n")?.is_empty());
		assert_eq!(
			commands("\u{feff}#NEXUS\nTITLE 'Árbol';")?[0].0,
			"TITLE 'Árbol';"
		);
		Ok(())
	}

	#[test]
	fn ignores_comment_only_input() -> Result<()> {
		assert_eq!(
			commands(
				"#NEXUS\n[before]; BEGIN NOTES; END; [after]"
			)?,
			[
				("BEGIN NOTES;".to_owned(), 2, 11),
				("END;".to_owned(), 2, 24),
			]
		);
		Ok(())
	}

	#[test]
	fn rejects_invalid_input() {
		for (input, message) in [
			("BEGIN TREES;", "Expected '#NEXUS'"),
			("#NEXUSx\n", "Expected whitespace"),
			(
				"#NEXUS\nTREE x = (A,B)",
				"Unterminated NEXUS command",
			),
			("#NEXUS\nTREE x = ('A,B);", "Unterminated quoted"),
			(
				"#NEXUS\nTREE x = (A[broken,B);",
				"Unterminated NEXUS comment",
			),
			("#NEXUS\nTREE x = (A],B);", "Unexpected ']'"),
		] {
			let error = commands(input).unwrap_err().to_string();
			assert!(
				error.contains(message),
				"expected {message:?} in {error:?}"
			);
		}
	}
}
