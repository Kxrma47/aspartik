use anyhow::{Result, bail, ensure};

use std::{collections::HashSet, io::BufRead};

use crate::TaxonSet;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
	Outside,
	Trees,
	Other,
}

struct NexusParser {
	block: Block,
	translation: Option<(TaxonSet, TaxonSet)>,
	saw_tree: bool,
	quote: Option<u8>,
	comment_depth: usize,
}

impl Default for NexusParser {
	fn default() -> Self {
		Self {
			block: Block::Outside,
			translation: None,
			saw_tree: false,
			quote: None,
			comment_depth: 0,
		}
	}
}

fn update_comment_depth(depth: &mut usize, byte: u8) {
	match byte {
		b'[' => *depth += 1,
		b']' => *depth -= 1,
		_ => {}
	}
}

impl NexusParser {
	fn find(&mut self, input: &str, target: u8) -> Option<usize> {
		let bytes = input.as_bytes();
		let mut index = 0;
		while index < bytes.len() {
			let current = bytes[index];
			if self.comment_depth != 0 {
				update_comment_depth(
					&mut self.comment_depth,
					current,
				);
			} else if let Some(quote) = self.quote {
				if current == quote {
					if bytes.get(index + 1) == Some(&quote)
					{
						index += 1;
					} else {
						self.quote = None;
					}
				}
			} else {
				match current {
					b'[' => update_comment_depth(
						&mut self.comment_depth,
						current,
					),
					b'\'' | b'"' => {
						self.quote = Some(current)
					}
					_ if current == target => {
						return Some(index);
					}
					_ => {}
				}
			}
			index += 1;
		}
		None
	}

	fn consume_comment<'a>(&mut self, input: &'a str) -> &'a str {
		let mut end = 0;
		for (index, byte) in input.bytes().enumerate() {
			update_comment_depth(&mut self.comment_depth, byte);
			end = index + 1;
			if self.comment_depth == 0 {
				break;
			}
		}
		&input[end..]
	}

	fn skip_trivia<'a>(&mut self, mut input: &'a str) -> &'a str {
		loop {
			if self.comment_depth == 0 {
				input = input.trim_start();
			}
			if self.comment_depth == 0 && !input.starts_with('[') {
				return input;
			}
			input = self.consume_comment(input);
			if self.comment_depth != 0 {
				return input;
			}
		}
	}

	fn after_keyword<'a>(
		&mut self,
		input: &'a str,
		keyword: &str,
	) -> Option<&'a str> {
		let input = self.skip_trivia(input);
		let word = input.get(..keyword.len())?;
		if !word.eq_ignore_ascii_case(keyword) {
			return None;
		}
		let rest = &input[keyword.len()..];
		if rest.chars().next().is_some_and(|character| {
			!character.is_whitespace()
				&& !matches!(character, ';' | '[' | '*')
		}) {
			return None;
		}
		Some(rest)
	}

	fn word(&mut self, input: &mut &str) -> Result<Option<String>> {
		*input = self.skip_trivia(input);
		let mut value = String::new();
		while let Some(character) = (*input).chars().next() {
			if character.is_whitespace()
				|| matches!(character, ',' | ';')
			{
				break;
			}
			if character == '[' {
				*input = self.consume_comment(input);
				ensure!(
					self.comment_depth == 0,
					"Unterminated comment"
				);
				continue;
			}
			if matches!(character, '\'' | '"') {
				*input = &(*input)[1..];
				loop {
					let next = (*input)
						.chars()
						.next()
						.ok_or_else(|| {
							anyhow::anyhow!(
								"Unterminated quoted name"
							)
						})?;
					*input = &(*input)[next.len_utf8()..];
					if next == character {
						if (*input)
							.starts_with(character)
						{
							*input = &(*input)[1..];
							value.push(character);
						} else {
							break;
						}
					} else {
						value.push(next);
					}
				}
				continue;
			}
			ensure!(
				character != ']',
				"Unexpected ']' in TRANSLATE"
			);
			value.push(character);
			*input = &(*input)[character.len_utf8()..];
		}
		Ok((!value.is_empty()).then_some(value))
	}

	fn required_word(
		&mut self,
		input: &mut &str,
		expected: &str,
	) -> Result<String> {
		self.word(input)?
			.ok_or_else(|| anyhow::anyhow!("Expected {expected}"))
	}

	fn parse_translation(
		&mut self,
		mut input: &str,
	) -> Result<(TaxonSet, TaxonSet)> {
		let mut pairs = Vec::new();
		let mut aliases = HashSet::new();
		let mut taxa = HashSet::new();
		loop {
			let alias = self.required_word(
				&mut input,
				"a translation key",
			)?;
			let name =
				self.required_word(&mut input, "a taxon name")?;
			ensure!(
				aliases.insert(alias.clone()),
				"Duplicate translation key: {alias}"
			);
			ensure!(
				taxa.insert(name.clone()),
				"Duplicate taxon name: {name}"
			);
			pairs.push((alias, name));
			input = self.skip_trivia(input);
			let delimiter =
				input.as_bytes().first().copied().ok_or_else(
					|| {
						anyhow::anyhow!(
							"Expected ',' or ';' after a translation entry"
						)
					},
				)?;
			ensure!(
				matches!(delimiter, b',' | b';'),
				"Expected ',' or ';' after a translation entry"
			);
			input = &input[1..];
			if delimiter == b';' {
				break;
			}
		}
		ensure!(
			self.skip_trivia(input).is_empty(),
			"Unexpected input after TRANSLATE"
		);
		pairs.sort_unstable_by(|first, second| first.1.cmp(&second.1));
		Ok((
			TaxonSet::from_iter(
				pairs.iter().map(|(alias, _)| alias.as_str()),
			),
			TaxonSet::from_iter(
				pairs.iter().map(|(_, name)| name.as_str()),
			),
		))
	}

	fn process<F>(
		&mut self,
		statement: &str,
		callback: &mut F,
	) -> Result<()>
	where
		F: FnMut(&str, Option<(&TaxonSet, &TaxonSet)>) -> Result<()>,
	{
		let mut statement = self.skip_trivia(statement);
		if let Some(rest) = self.after_keyword(statement, "#NEXUS") {
			statement = self.skip_trivia(rest);
		}
		if statement.is_empty() {
			return Ok(());
		}
		if self.block == Block::Outside {
			if let Some(rest) =
				self.after_keyword(statement, "BEGIN")
			{
				self.block = if self
					.after_keyword(rest, "TREES")
					.is_some()
				{
					Block::Trees
				} else {
					Block::Other
				};
				self.translation = None;
				self.saw_tree = false;
			}
			return Ok(());
		}
		if self.after_keyword(statement, "END").is_some()
			|| self.after_keyword(statement, "ENDBLOCK").is_some()
		{
			self.block = Block::Outside;
			self.translation = None;
			self.saw_tree = false;
			return Ok(());
		}
		if self.block == Block::Other {
			return Ok(());
		}
		if let Some(rest) = self.after_keyword(statement, "TRANSLATE") {
			ensure!(
				!self.saw_tree,
				"TRANSLATE must precede TREE commands"
			);
			ensure!(
				self.translation.is_none(),
				"A TREES block has more than one TRANSLATE command"
			);
			self.translation = Some(self.parse_translation(rest)?);
			return Ok(());
		}
		let Some(rest) = self
			.after_keyword(statement, "TREE")
			.or_else(|| self.after_keyword(statement, "UTREE"))
		else {
			return Ok(());
		};
		let rest = self.skip_trivia(rest);
		let rest = self
			.skip_trivia(rest.strip_prefix('*').unwrap_or(rest));
		let equal = self.find(rest, b'=').ok_or_else(|| {
			anyhow::anyhow!("Expected '=' in TREE command")
		})?;
		ensure!(
			!rest[..equal].trim().is_empty(),
			"Expected a tree name"
		);
		let newick = rest[equal + 1..].trim();
		self.saw_tree = true;
		callback(
			newick,
			self.translation
				.as_ref()
				.map(|(aliases, taxa)| (aliases, taxa)),
		)
	}
}

pub fn for_each_tree<R, F>(mut reader: R, mut callback: F) -> Result<()>
where
	R: BufRead,
	F: FnMut(&str, Option<(&TaxonSet, &TaxonSet)>) -> Result<()>,
{
	let mut parser = NexusParser::default();
	let mut line = String::new();
	let mut pending = String::new();
	while reader.read_line(&mut line)? != 0 {
		let mut rest = line.as_str();
		while !rest.is_empty() {
			if let Some(end) = parser.find(rest, b';') {
				let (part, tail) = rest.split_at(end + 1);
				if pending.is_empty() {
					parser.process(part, &mut callback)?;
				} else {
					pending.push_str(part);
					parser.process(
						&pending,
						&mut callback,
					)?;
					pending.clear();
				}
				rest = tail;
			} else {
				pending.push_str(rest);
				break;
			}
		}
		line.clear();
	}
	if parser.quote.is_some() {
		bail!("Unterminated quote in NEXUS input");
	}
	if parser.comment_depth != 0 {
		bail!("Unterminated comment in NEXUS input");
	}
	ensure!(
		pending.trim().is_empty()
			|| pending.trim().eq_ignore_ascii_case("#NEXUS"),
		"Unterminated NEXUS command"
	);
	ensure!(parser.block == Block::Outside, "Unterminated NEXUS block");
	Ok(())
}
