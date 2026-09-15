use anyhow::Result;
use arbitrary::Unstructured;
use arbtest::arbtest;

use std::{
	io::{BufReader, Cursor},
	path::PathBuf,
};

use data::nexus::{CommandReader, NexusTreeReader, parse_trees};

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

fn fixture(name: &str) -> PathBuf {
	PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("tests/fixtures/nexus")
		.join(name)
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

#[test]
fn beast_trees() -> Result<()> {
	let trees = NexusTreeReader::from_path(fixture("beast.nex"))?
		.collect::<Result<Vec<_>>>()?;
	assert_eq!(trees.len(), 2);
	assert_eq!(trees[0].name(), "STATE_0");
	assert_eq!(trees[0].is_rooted(), Some(true));
	assert_eq!(trees[0].tree().hybrid_edges().count(), 0);
	assert_eq!(
		trees[0].tree().to_newick()?,
		"((A[&date=2020]:0.1[&rate=0.5],B:0.2)[&posterior=0.9]:0.3,'C C':0.4)[&R];"
	);
	assert!(trees[0].tree().clone().into_binary().is_ok());
	assert!(trees[1].tree().clone().into_binary().is_ok());
	Ok(())
}

#[test]
fn mrbayes_and_paup_trees() -> Result<()> {
	let mrbayes = NexusTreeReader::from_path(fixture("mrbayes.nex"))?
		.collect::<Result<Vec<_>>>()?;
	assert_eq!(mrbayes.len(), 2);
	assert_eq!(mrbayes[0].is_rooted(), Some(false));
	assert!(mrbayes[1].is_default());
	assert_eq!(mrbayes[1].tree().to_newick()?, "((A:1,C:1):1,B:2)[&R];");

	let paup = NexusTreeReader::from_path(fixture("paup.nex"))?
		.collect::<Result<Vec<_>>>()?;
	assert_eq!(paup.len(), 1);
	assert_eq!(paup[0].name(), "PAUP_1");
	assert!(paup[0].is_default());
	assert_eq!(paup[0].tree().children_of(paup[0].tree().root()).len(), 4);
	Ok(())
}

#[test]
fn parses_across_small_reader_buffers() -> Result<()> {
	let source = include_bytes!("../fixtures/nexus/beast.nex");
	for capacity in 1..=16 {
		let reader =
			BufReader::with_capacity(capacity, Cursor::new(source));
		let trees = NexusTreeReader::new(reader)?
			.collect::<Result<Vec<_>>>()?;
		assert_eq!(trees.len(), 2);
	}
	Ok(())
}

#[test]
fn streams_many_trees() -> Result<()> {
	const NUM_TREES: usize = 10_000;
	let mut source = String::from("#NEXUS\nBEGIN TREES;\n");
	for index in 0..NUM_TREES {
		source.push_str(&format!("TREE tree_{index} = (A:1,B:1);\n"));
	}
	source.push_str("END;\n");

	let mut reader = NexusTreeReader::new(Cursor::new(source))?;
	for index in 0..NUM_TREES {
		let tree = reader.next_tree()?.unwrap();
		assert_eq!(tree.name(), format!("tree_{index}"));
		assert_eq!(tree.tree().num_nodes(), 3);
	}
	assert!(reader.next_tree()?.is_none());
	Ok(())
}

#[test]
fn reports_file_and_syntax_errors() {
	let missing = NexusTreeReader::from_path(fixture("missing.nex"))
		.unwrap_err()
		.to_string();
	assert!(missing.contains("Could not open NEXUS file"));

	for (source, message) in [
		("#NEXUS\nBEGIN TREES; TREE x=(A,B);", "was not closed"),
		(
			"#NEXUS\nBEGIN TREES; TREE x=(A,B]C); END;",
			"Unexpected ']'",
		),
		(
			"#NEXUS\nBEGIN TREES; TREE x=(A,B) root extra; END;",
			"more than one label",
		),
	] {
		let error = format!("{:#}", parse_trees(source).unwrap_err());
		assert!(
			error.contains(message),
			"expected {message:?} in {error:?}"
		);
	}
}

#[test]
fn arbitrary_input_does_not_panic() {
	arbtest(|u: &mut Unstructured<'_>| {
		let length = u.int_in_range(0_usize..=10_000)?;
		let bytes = u.bytes(length)?;
		let mut source = String::from("#NEXUS\n");
		source.extend(bytes
			.iter()
			.map(|byte| char::from(32 + byte % 95)));
		let _ = parse_trees(&source);
		Ok(())
	})
	.size_min(2_u32.pow(20));
}
