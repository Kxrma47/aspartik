use anyhow::Result;
use arbitrary::Unstructured;
use arbtest::arbtest;

use data::tree::{BinaryTree, Node, builder::TreeBuilder};

fn add(tree: &mut TreeBuilder, parent: Node, name: &str, length: f64) -> Node {
	tree.add_node(name, length, parent)
}

fn balanced_builder_4_leaves() -> Result<TreeBuilder> {
	let mut tree = TreeBuilder::new();
	let root = tree.add_dummy_node();
	tree.set_root(root);
	add(&mut tree, root, "left", 1.0);
	add(&mut tree, root, "right", 2.0);
	let left = tree.children_of(root)[0];
	add(&mut tree, left, "a", 3.0);
	add(&mut tree, left, "b", 4.0);
	Ok(tree)
}

fn binary_node_by_name(tree: &BinaryTree, name: &str) -> Node {
	tree.nodes()
		.find(|&node| tree.name(node) == Some(name))
		.unwrap()
}

fn builder_node_by_name(tree: &TreeBuilder, name: &str) -> Node {
	tree.nodes().find(|&node| tree.name(node) == name).unwrap()
}

#[test]
fn deterministic_binary_shapes() -> Result<()> {
	let mut two_tip = TreeBuilder::new();
	let root = two_tip.add_dummy_node();
	two_tip.set_root(root);
	add(&mut two_tip, root, "a", 1.0);
	add(&mut two_tip, root, "b", 2.0);

	let balanced = balanced_builder_4_leaves()?;

	for (builder, leaves, nodes) in [(two_tip, 2, 3), (balanced, 3, 5)] {
		assert!(builder.is_binary());
		let tree = builder.into_binary()?;
		assert_eq!(tree.num_leaves(), leaves);
		assert_eq!(tree.num_nodes(), nodes);
		assert_eq!(tree.preorder().count(), nodes as usize);
		assert_eq!(tree.postorder().count(), nodes as usize);
	}

	Ok(())
}

#[test]
fn binary_sealing_preserves_node_and_edge_data() -> Result<()> {
	let tree = balanced_builder_4_leaves()?;
	let binary = tree.clone().into_binary()?;

	for node in tree.nodes() {
		let name = tree.name(node);
		let target = binary_node_by_name(&binary, name);
		assert_eq!(binary.edge_length(target), tree.edge_length(node));
	}

	Ok(())
}

fn arbitrary_builder(
	u: &mut Unstructured<'_>,
	num_leaves: u32,
) -> arbitrary::Result<TreeBuilder> {
	let mut tree = TreeBuilder::new();
	let root = tree.add_dummy_node();
	tree.set_root(root);
	let mut leaves = vec![root];

	while leaves.len() < num_leaves as usize {
		let index = u.int_in_range(0..=leaves.len() - 1)?;
		let parent = leaves.swap_remove(index);
		for _ in 0..2 {
			let id = tree.num_nodes() as u32;
			let child = tree.add_node(
				&id.to_string(),
				f64::from(id) / 10.0,
				parent,
			);
			leaves.push(child);
		}
	}

	Ok(tree)
}

#[test]
fn random_binary_builder_roundtrips() {
	arbtest(|u: &mut Unstructured<'_>| {
		let num_leaves = u.int_in_range(2_u32..=128)?;
		let builder = arbitrary_builder(u, num_leaves)?;
		assert!(builder.is_binary());

		let binary = builder.clone().into_binary().unwrap();
		assert_eq!(binary.num_leaves(), num_leaves);
		assert_eq!(binary.num_nodes(), builder.num_nodes() as u32);
		assert_eq!(
			binary.preorder().count(),
			builder.num_nodes() as usize
		);
		assert_eq!(
			binary.postorder().count(),
			builder.num_nodes() as usize
		);

		for source in builder.nodes() {
			let name = builder.name(source);
			let target = binary_node_by_name(&binary, name);
			assert_eq!(
				binary.edge_length(target),
				builder.edge_length(source)
			);
			if builder.root() == Some(source) {
				continue;
			}
			let source_parent = builder.parent_of(source);
			let target_parent = binary.parent_of(target).unwrap();
			assert_eq!(
				binary.name(target_parent.into()),
				Some(builder.name(source_parent))
			);
		}

		Ok(())
	});
}

#[test]
fn node_by_name() -> Result<()> {
	let tree = balanced_builder_4_leaves()?;
	assert_eq!(
		builder_node_by_name(&tree, "left").u32(),
		tree.children_of(tree.root().unwrap())[0].u32()
	);

	Ok(())
}
