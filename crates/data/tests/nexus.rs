use anyhow::Result;

use std::io::Cursor;

use data::nexus::for_each_tree;

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
fn reads_multiline_nested_comments() -> Result<()> {
	let source = "#NEXUS\n[before\n[nested;]\nafter] BEGIN TREES; TREE [name\n[part;]\n] one = (A:1,B:2); END;";
	let mut trees = Vec::new();
	for_each_tree(Cursor::new(source), |tree| {
		trees.push(tree.to_owned());
		Ok(())
	})?;
	assert_eq!(trees, ["(A:1,B:2);"]);
	Ok(())
}

#[test]
fn passes_unparsed_newick_to_callback() -> Result<()> {
	let source = "#NEXUS\nBEGIN TREES; TREE empty = ; END;";
	let mut trees = Vec::new();
	for_each_tree(Cursor::new(source), |tree| {
		trees.push(tree.to_owned());
		Ok(())
	})?;
	assert_eq!(trees, [";"]);
	Ok(())
}

#[test]
fn rejects_truncated_input() {
	for source in [
		"#NEXUS\nBEGIN TREES; TREE x = (A:1,B:2)",
		"#NEXUS\nBEGIN TREES; TREE x = (A:1,B:2);",
		"#NEXUS\nBEGIN TREES; TREE x = ('A:1,B:2); END;",
		"#NEXUS\nBEGIN TREES; [unterminated\nTREE x = (A:1,B:2); END;",
	] {
		assert!(for_each_tree(Cursor::new(source), |_| Ok(())).is_err());
	}
}
