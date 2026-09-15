use anyhow::Result;

use std::io::{BufReader, Cursor};

use data::nexus::CommandReader;

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
		commands("#nexus BEGIN TREES;\n  TREE one = (A,B); END;")?,
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
	assert_eq!(commands[0].0, "[outer; [inner;] done] BEGIN TREES;");
	assert_eq!(commands[1].0, "TREE 'a;''b' = ('x;y',A[&note='z;']);");
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
		commands("#NEXUS\n[before]; BEGIN NOTES; END; [after]")?,
		[
			("BEGIN NOTES;".to_owned(), 2, 11),
			("END;".to_owned(), 2, 24),
		]
	);
	Ok(())
}

#[test]
fn reads_across_small_buffers() -> Result<()> {
	let input = "#NEXUS\nBEGIN TREES;\nTREE one = (Á,'B;B');\nEND;";
	let expected = commands(input)?;
	for capacity in 1..=16 {
		let reader =
			BufReader::with_capacity(capacity, Cursor::new(input));
		let observed = CommandReader::new(reader)?
			.map(|command| {
				let command = command?;
				Ok((
					command.source().to_owned(),
					command.line(),
					command.column(),
				))
			})
			.collect::<Result<Vec<_>>>()?;
		assert_eq!(observed, expected);
	}
	Ok(())
}

#[test]
fn rejects_invalid_input() {
	for (input, message) in [
		("BEGIN TREES;", "Expected '#NEXUS'"),
		("#NEXUSx\n", "Expected whitespace"),
		("#NEXUS\nTREE x = (A,B)", "Unterminated NEXUS command"),
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
