use anyhow::{Result, bail, ensure};

use std::io::BufRead;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
	Outside,
	Trees,
	Other,
}

#[derive(Default)]
struct Scanner {
	quote: Option<u8>,
	comment_depth: usize,
}

impl Scanner {
	fn find(&mut self, input: &str, target: u8) -> Option<usize> {
		let bytes = input.as_bytes();
		let mut index = 0;
		while index < bytes.len() {
			let current = bytes[index];
			if self.comment_depth != 0 {
				match current {
					b'[' => self.comment_depth += 1,
					b']' => self.comment_depth -= 1,
					_ => {}
				}
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
					b'[' => self.comment_depth = 1,
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
}

fn skip_trivia(mut input: &str) -> &str {
	loop {
		input = input.trim_start();
		if !input.starts_with('[') {
			return input;
		}
		let mut depth = 0;
		for (index, byte) in input.bytes().enumerate() {
			match byte {
				b'[' => depth += 1,
				b']' => {
					depth -= 1;
					if depth == 0 {
						input = &input[index + 1..];
						break;
					}
				}
				_ => {}
			}
		}
	}
}

fn after_keyword<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
	let input = skip_trivia(input);
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

fn process<F>(
	statement: &str,
	block: &mut Block,
	callback: &mut F,
) -> Result<()>
where
	F: FnMut(&str) -> Result<()>,
{
	let mut statement = skip_trivia(statement);
	if let Some(rest) = after_keyword(statement, "#NEXUS") {
		statement = skip_trivia(rest);
	}
	if statement.is_empty() {
		return Ok(());
	}
	if *block == Block::Outside {
		if let Some(rest) = after_keyword(statement, "BEGIN") {
			*block = if after_keyword(rest, "TREES").is_some() {
				Block::Trees
			} else {
				Block::Other
			};
		}
		return Ok(());
	}
	if after_keyword(statement, "END").is_some()
		|| after_keyword(statement, "ENDBLOCK").is_some()
	{
		*block = Block::Outside;
		return Ok(());
	}
	if *block == Block::Other {
		return Ok(());
	}
	let Some(rest) = after_keyword(statement, "TREE")
		.or_else(|| after_keyword(statement, "UTREE"))
	else {
		return Ok(());
	};
	let rest = skip_trivia(rest);
	let rest = skip_trivia(rest.strip_prefix('*').unwrap_or(rest));
	let equal = Scanner::default().find(rest, b'=').ok_or_else(|| {
		anyhow::anyhow!("Expected '=' in TREE command")
	})?;
	ensure!(!rest[..equal].trim().is_empty(), "Expected a tree name");
	let newick = rest[equal + 1..].trim();
	ensure!(newick != ";", "Expected a Newick tree");
	callback(newick)
}

pub fn for_each_tree<R, F>(mut reader: R, mut callback: F) -> Result<()>
where
	R: BufRead,
	F: FnMut(&str) -> Result<()>,
{
	let mut block = Block::Outside;
	let mut line = String::new();
	let mut pending = String::new();
	let mut scanner = Scanner::default();
	while reader.read_line(&mut line)? != 0 {
		let mut rest = line.as_str();
		while !rest.is_empty() {
			if let Some(end) = scanner.find(rest, b';') {
				let (part, tail) = rest.split_at(end + 1);
				if pending.is_empty() {
					process(
						part,
						&mut block,
						&mut callback,
					)?;
				} else {
					pending.push_str(part);
					process(
						&pending,
						&mut block,
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
	if scanner.quote.is_some() {
		bail!("Unterminated quote in NEXUS input");
	}
	if scanner.comment_depth != 0 {
		bail!("Unterminated comment in NEXUS input");
	}
	ensure!(
		pending.trim().is_empty()
			|| pending.trim().eq_ignore_ascii_case("#NEXUS"),
		"Unterminated NEXUS command"
	);
	ensure!(block == Block::Outside, "Unterminated NEXUS block");
	Ok(())
}

#[cfg(test)]
mod tests {
	use anyhow::Result;

	use std::io::Cursor;

	use super::for_each_tree;

	#[test]
	fn reads_only_tree_blocks() -> Result<()> {
		let source = "#NEXUS\nBEGIN TAXA; TREE ignored = (X:1,Y:1); END;\nBEGIN TREES; TREE * first = [&R](A:1,B:2); UTREE second =\n(A:3,B:4); END;";
		let mut trees = Vec::new();
		for_each_tree(Cursor::new(source), |tree| {
			trees.push(tree.to_owned());
			Ok(())
		})?;
		assert_eq!(trees, ["[&R](A:1,B:2);", "(A:3,B:4);"]);
		Ok(())
	}

	#[test]
	fn ignores_semicolons_inside_quotes_and_comments() -> Result<()> {
		let source = "#NEXUS\nBEGIN TREES; TREE 'name;one' = ('A;B'[note;inside]:1,C:2); END;";
		let mut trees = Vec::new();
		for_each_tree(Cursor::new(source), |tree| {
			trees.push(tree.to_owned());
			Ok(())
		})?;
		assert_eq!(trees, ["('A;B'[note;inside]:1,C:2);"]);
		Ok(())
	}

	#[test]
	fn rejects_truncated_input() {
		for source in [
			"#NEXUS\nBEGIN TREES; TREE x = (A:1,B:2)",
			"#NEXUS\nBEGIN TREES; TREE x = (A:1,B:2);",
			"#NEXUS\nBEGIN TREES; TREE x = ('A:1,B:2); END;",
		] {
			assert!(for_each_tree(Cursor::new(source), |_| Ok(()))
				.is_err());
		}
	}
}
