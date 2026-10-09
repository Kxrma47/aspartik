use anyhow::{Result, bail, ensure};

use std::collections::HashSet;

use crate::TaxonSet;

use super::after_keyword;

struct Parser<'a> {
	input: &'a str,
	index: usize,
}

impl<'a> Parser<'a> {
	fn new(input: &'a str) -> Self {
		Self { input, index: 0 }
	}

	fn skip_trivia(&mut self) -> Result<()> {
		loop {
			while let Some(character) =
				self.input[self.index..].chars().next()
			{
				if !character.is_whitespace() {
					break;
				}
				self.index += character.len_utf8();
			}
			if self.input.as_bytes().get(self.index) != Some(&b'[')
			{
				return Ok(());
			}
			self.skip_comment()?;
		}
	}

	fn skip_comment(&mut self) -> Result<()> {
		let mut depth = 0;
		while let Some(byte) = self.input.as_bytes().get(self.index) {
			self.index += 1;
			match byte {
				b'[' => depth += 1,
				b']' => {
					depth -= 1;
					if depth == 0 {
						return Ok(());
					}
				}
				_ => {}
			}
		}
		bail!("Unterminated comment")
	}

	fn word(&mut self) -> Result<Option<String>> {
		self.skip_trivia()?;
		let mut value = String::new();
		while let Some(character) =
			self.input[self.index..].chars().next()
		{
			if character.is_whitespace()
				|| matches!(character, ',' | ';')
			{
				break;
			}
			if character == '[' {
				self.skip_comment()?;
				continue;
			}
			if matches!(character, '\'' | '"') {
				self.index += character.len_utf8();
				loop {
					let next = self.input[self.index..]
						.chars()
						.next()
						.ok_or_else(|| {
							anyhow::anyhow!(
								"Unterminated quoted name"
							)
						})?;
					self.index += next.len_utf8();
					if next == character {
						if self.input[self.index..]
							.starts_with(character)
						{
							self.index += character
								.len_utf8();
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
			self.index += character.len_utf8();
		}
		Ok((!value.is_empty()).then_some(value))
	}

	fn required_word(&mut self, expected: &str) -> Result<String> {
		self.word()?
			.ok_or_else(|| anyhow::anyhow!("Expected {expected}"))
	}

	fn delimiter(&mut self) -> Result<u8> {
		self.skip_trivia()?;
		let byte = self.input.as_bytes().get(self.index).copied();
		ensure!(
			matches!(byte, Some(b',' | b';')),
			"Expected ',' or ';' after a translation entry"
		);
		self.index += 1;
		Ok(byte.unwrap())
	}
}

pub(super) fn parse(statement: &str) -> Result<(TaxonSet, TaxonSet)> {
	let rest = after_keyword(statement, "TRANSLATE")
		.ok_or_else(|| anyhow::anyhow!("Expected TRANSLATE"))?;
	let mut parser = Parser::new(rest);
	let mut pairs = Vec::new();
	let mut aliases = HashSet::new();
	let mut taxa = HashSet::new();
	loop {
		let alias = parser.required_word("a translation key")?;
		let name = parser.required_word("a taxon name")?;
		ensure!(
			aliases.insert(alias.clone()),
			"Duplicate translation key: {alias}"
		);
		ensure!(
			taxa.insert(name.clone()),
			"Duplicate taxon name: {name}"
		);
		pairs.push((alias, name));
		match parser.delimiter()? {
			b',' => continue,
			b';' => break,
			_ => bail!("Expected ',' or ';'"),
		}
	}
	parser.skip_trivia()?;
	ensure!(
		parser.index == rest.len(),
		"Unexpected input after TRANSLATE"
	);
	pairs.sort_unstable_by(|first, second| first.1.cmp(&second.1));
	let aliases = TaxonSet::from_iter(
		pairs.iter().map(|(alias, _)| alias.as_str()),
	);
	let taxa = TaxonSet::from_iter(
		pairs.iter().map(|(_, name)| name.as_str()),
	);
	Ok((aliases, taxa))
}
