use anyhow::Result;

use std::io::Cursor;

use data::nexus::for_each_tree;

#[test]
fn reads_only_tree_blocks() -> Result<()> {
	let source = "#NEXUS\nBEGIN TAXA; TREE ignored = (X:1,Y:1); END;\nBEGIN TREES; TREE * first = [&R](A:1,B:2); UTREE second =\n(A:3,B:4); END;";
	let mut trees = Vec::new();
	for_each_tree(Cursor::new(source), |tree, translation| {
		assert!(translation.is_none());
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
	for_each_tree(Cursor::new(source), |tree, translation| {
		assert!(translation.is_none());
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
	for_each_tree(Cursor::new(source), |tree, translation| {
		assert!(translation.is_none());
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
	for_each_tree(Cursor::new(source), |tree, translation| {
		assert!(translation.is_none());
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
		assert!(for_each_tree(Cursor::new(source), |_, _| Ok(()))
			.is_err());
	}
}

#[test]
fn passes_translations_and_resets_them() -> Result<()> {
	let source = "#NEXUS\nBEGIN TREES;\nTRANSLATE 2 'B B',\n1 A, 3 'O''Brien';\nTREE first = (1:1,(2:2,3:3):4);\nEND;\nBEGIN TREES; TREE second = (A:1,B:1); END;";
	let mut count = 0;
	for_each_tree(Cursor::new(source), |tree, translation| {
		match count {
			0 => {
				let (aliases, taxa) = translation.unwrap();
				assert_eq!(
					aliases.iter().collect::<Vec<_>>(),
					["1", "2", "3"]
				);
				assert_eq!(
					taxa.iter().collect::<Vec<_>>(),
					["A", "B B", "O'Brien"]
				);
				assert_eq!(tree, "(1:1,(2:2,3:3):4);");
			}
			1 => {
				assert!(translation.is_none());
				assert_eq!(tree, "(A:1,B:1);");
			}
			_ => panic!("Unexpected tree"),
		}
		count += 1;
		Ok(())
	})?;
	assert_eq!(count, 2);
	Ok(())
}

#[test]
fn parses_comments_and_quoted_names_in_translation() -> Result<()> {
	let source = "#NEXUS\nBEGIN TREES;\nTRANSLATE [outer\n[inner;]\n] 2 'B;B', 1[inline] 'A A';\nTREE first = (1:1,2:2); END;";
	let mut count = 0;
	for_each_tree(Cursor::new(source), |tree, translation| {
		let (aliases, taxa) = translation.unwrap();
		assert_eq!(aliases.iter().collect::<Vec<_>>(), ["1", "2"]);
		assert_eq!(taxa.iter().collect::<Vec<_>>(), ["A A", "B;B"]);
		assert_eq!(tree, "(1:1,2:2);");
		count += 1;
		Ok(())
	})?;
	assert_eq!(count, 1);
	Ok(())
}

#[test]
fn rejects_invalid_translation() {
	for source in [
		"#NEXUS\nBEGIN TREES; TRANSLATE 1 A, 1 B; END;",
		"#NEXUS\nBEGIN TREES; TRANSLATE 1 A, 2 A; END;",
		"#NEXUS\nBEGIN TREES; TRANSLATE 1; END;",
		"#NEXUS\nBEGIN TREES; TRANSLATE 1 A,; END;",
		"#NEXUS\nBEGIN TREES; TRANSLATE 1 A; TRANSLATE 2 B; END;",
		"#NEXUS\nBEGIN TREES; TREE t=(A:1,B:1); TRANSLATE 1 A; END;",
	] {
		assert!(for_each_tree(Cursor::new(source), |_, _| Ok(()))
			.is_err());
	}
}
