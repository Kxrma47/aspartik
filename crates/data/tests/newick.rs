use anyhow::Result;
use arbitrary::{Arbitrary, Unstructured};
use arbtest::arbtest;

use data::tree::{
	BinaryTree, Node,
	builder::{EdgeData, NodeData, TreeBuilder},
	parse_newick,
};

fn node_named(tree: &TreeBuilder, name: &str) -> Node {
	tree.nodes()
		.find(|&node| tree.node(node).unwrap().name == name)
		.unwrap()
}

#[test]
fn labels_lengths_and_empty_fields() -> Result<()> {
	for (source, expected) in [
		(
			"(A:0.1,B:2e-3,(C,D)CD:4E+2)ROOT;",
			"(A:0.1,B:0.002,(C,D)CD:400)ROOT;",
		),
		("(,A,,)ROOT;", "(,A,,)ROOT;"),
		("single;", "single;"),
	] {
		let tree = parse_newick(source)?;
		assert_eq!(tree.to_newick()?, expected);
	}

	let tree = parse_newick("(A,B:1);")?;
	assert_eq!(tree.edge(node_named(&tree, "A")).unwrap().length, None);
	assert!(tree.into_binary().is_err());

	Ok(())
}

#[test]
fn quoted_labels() -> Result<()> {
	let tree = parse_newick(
		"('with spaces':0.1,'O''Brien':0.2,'comma,name':0.3);",
	)?;
	assert_eq!(
		tree.to_newick()?,
		"('with spaces':0.1,'O''Brien':0.2,'comma,name':0.3);"
	);
	assert_eq!(
		tree.node(node_named(&tree, "O'Brien")).unwrap().name,
		"O'Brien"
	);

	let double_quoted =
		parse_newick("(\"with quotes!\":0.1,\"another one\":0.2);")?;
	assert_eq!(
		double_quoted.to_newick()?,
		"('with quotes!':0.1,'another one':0.2);"
	);

	Ok(())
}

#[test]
fn beast_metadata_and_nhx() -> Result<()> {
	let source = "[&R](A[&date=2020]:1e-2[&rate=0.5],B:0.02)ROOT[&&NHX:S=human:broken:D=N][&posterior=0.99];";
	let expected = "(A[&date=2020]:0.01[&rate=0.5],B:0.02)ROOT[&R][&&NHX:S=human:broken:D=N][&posterior=0.99];";
	let builder = parse_newick(source)?;
	let a = node_named(&builder, "A");
	assert_eq!(builder.node(a).unwrap().attributes, "[&date=2020]");
	assert_eq!(builder.edge(a).unwrap().attributes, "[&rate=0.5]");
	assert_eq!(builder.to_newick()?, expected);

	let tree = builder.into_binary()?;
	let root = Node::from(tree.root());
	assert_eq!(tree.nhx(root, "S"), Some("human"));
	assert_eq!(tree.nhx(root, "D"), Some("N"));
	assert_eq!(tree.nhx(root, "missing"), None);
	assert_eq!(tree.to_newick()?, expected);

	Ok(())
}

#[test]
fn multifurcating_roundtrip() -> Result<()> {
	let source = "(A:1,B:2,C:3,D:4)ROOT;";
	let builder = parse_newick(source)?;
	assert!(!builder.is_binary());
	assert!(builder.clone().into_binary().is_err());
	assert_eq!(builder.children_of(builder.root()).unwrap().len(), 4);
	assert_eq!(builder.to_newick()?, source);

	Ok(())
}

#[test]
fn extended_newick_hybrid_roundtrip() -> Result<()> {
	let source = "(A:1,B:1,((C:1,(Y:1)x#H1[&kind=donor]:1)c:1,(x#H1:2[&&NHX:gamma=0.3],D:1)d:1)e:1)f;";
	let builder = parse_newick(source)?;
	assert_eq!(builder.hybrid_edges().count(), 1);
	let output = builder.to_newick()?;
	let reparsed = parse_newick(&output)?;
	assert_eq!(reparsed.hybrid_edges().count(), 1);
	assert_eq!(reparsed.num_nodes(), builder.num_nodes());

	Ok(())
}

#[test]
fn hybrid_definition_follows_additional_parent() -> Result<()> {
	let mut tree = TreeBuilder::with_root(NodeData::named("root"));
	let additional_parent = tree.add_node(
		tree.root(),
		NodeData::named("additional"),
		EdgeData::from_distance(1.0),
	)?;
	let canonical_parent = tree.add_node(
		tree.root(),
		NodeData::named("canonical"),
		EdgeData::from_distance(2.0),
	)?;
	let hybrid = tree.add_node(
		canonical_parent,
		NodeData::new("x#H1".to_owned(), "[&kind=donor]".to_owned()),
		EdgeData::from_distance(3.0),
	)?;
	tree.add_node(
		hybrid,
		NodeData::named("leaf"),
		EdgeData::from_distance(4.0),
	)?;
	tree.add_hybrid_edge(additional_parent, hybrid)?;

	let output = tree.to_newick()?;
	assert_eq!(output.matches("[&kind=donor]").count(), 1);
	let reparsed = parse_newick(&output)?;
	assert_eq!(reparsed.num_nodes(), tree.num_nodes());
	assert_eq!(reparsed.hybrid_edges().count(), 1);

	Ok(())
}

#[test]
fn malformed_and_truncated_input() {
	for source in [
		"",
		"(A:1,B:1)",
		"(A:1,B:1;",
		"A:wat;",
		"A:1:2;",
		"('A:1,B:1);",
		"(A[&x=1,B:1);",
		"();",
		"(A:1,B:1));",
		"(A:1,B:1); trailing",
		"((A:1)X#H:1,B:1);",
		"((A:1)X#:1,B:1);",
		"((A:1)X#H1:1,(X#H1:1,B:1)X#H1:1);",
	] {
		assert!(parse_newick(source).is_err(), "accepted {source:?}");
	}
}

#[test]
fn deep_ladder_parse_and_write() -> Result<()> {
	const NUM_LEAVES: usize = 20_000;
	let mut source = String::with_capacity(NUM_LEAVES * 16);
	for leaf in 0..NUM_LEAVES - 1 {
		source.push('(');
		source.push_str(&format!("L{leaf}:1,"));
	}
	source.push_str(&format!("L{}:1", NUM_LEAVES - 1));
	for _ in 0..NUM_LEAVES - 1 {
		source.push_str("):1");
	}
	source.push(';');

	let tree = parse_newick(&source)?;
	assert_eq!(tree.num_nodes(), (NUM_LEAVES * 2 - 1) as u32);
	let output = tree.to_newick()?;
	let reparsed = parse_newick(&output)?;
	assert_eq!(reparsed.num_nodes(), tree.num_nodes());
	assert!(reparsed.is_binary());

	Ok(())
}

fn arbitrary_builder(
	u: &mut Unstructured<'_>,
	num_leaves: u32,
) -> arbitrary::Result<TreeBuilder> {
	let mut tree = TreeBuilder::with_root(NodeData::named("root"));
	let mut leaves = vec![tree.root()];
	while leaves.len() < num_leaves as usize {
		let index = u.int_in_range(0..=leaves.len() - 1)?;
		let parent = leaves.swap_remove(index);
		for _ in 0..2 {
			let id = tree.num_nodes();
			let child = tree
				.add_node(
					parent,
					NodeData::new(
						format!("node {id}"),
						format!("[&id={id}]"),
					),
					EdgeData::new(
						Some(f64::from(id) / 10.0),
						format!("[&edge={id}]"),
					),
				)
				.unwrap();
			leaves.push(child);
		}
	}
	Ok(tree)
}

#[test]
fn random_parse_serialize_parse() {
	arbtest(|u: &mut Unstructured<'_>| {
		let num_leaves = u.int_in_range(2_u32..=128)?;
		let tree = arbitrary_builder(u, num_leaves)?;
		let first = tree.to_newick().unwrap();
		let reparsed = parse_newick(&first).unwrap();
		let second = reparsed.to_newick().unwrap();
		assert_eq!(first, second);
		let binary: BinaryTree = reparsed.into_binary().unwrap();
		assert_eq!(binary.num_leaves(), num_leaves);
		Ok(())
	});
}

#[test]
fn arbitrary_input_does_not_panic() {
	arbtest(|u: &mut Unstructured<'_>| {
		let input = String::arbitrary(u)?;
		let _ = parse_newick(&input);
		Ok(())
	});
}
