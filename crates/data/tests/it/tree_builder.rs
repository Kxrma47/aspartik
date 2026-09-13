use anyhow::Result;
use arbitrary::Unstructured;
use arbtest::arbtest;

use data::tree::{
	BinaryTree, Node,
	builder::{EdgeData, NodeData, TreeBuilder},
};

fn node_data(
	name: impl Into<String>,
	attributes: impl Into<String>,
) -> NodeData {
	NodeData::new(name.into(), attributes.into())
}

fn edge_data(length: f64, attributes: impl Into<String>) -> EdgeData {
	EdgeData::new(Some(length), attributes.into())
}

fn add(
	tree: &mut TreeBuilder,
	parent: Node,
	name: &str,
	length: f64,
) -> Result<Node> {
	tree.add_node(
		parent,
		node_data(name, format!("node_{name}")),
		edge_data(length, format!("edge_{name}")),
	)
}

fn balanced_builder_4_leaves() -> Result<TreeBuilder> {
	let mut tree = TreeBuilder::with_root(node_data("root", "node_root"));
	let root = tree.root();
	let left = add(&mut tree, root, "left", 1.0)?;
	let right = add(&mut tree, root, "right", 2.0)?;
	add(&mut tree, left, "a", 3.0)?;
	add(&mut tree, left, "b", 4.0)?;
	add(&mut tree, right, "c", 5.0)?;
	add(&mut tree, right, "d", 6.0)?;
	Ok(tree)
}

fn canonical_edges(tree: &TreeBuilder) -> Vec<(u32, u32, Option<u64>, String)> {
	let mut edges = tree
		.nodes()
		.filter_map(|child| {
			let parent = tree.parent_of(child)?;
			let edge = tree.edge(child);
			Some((
				parent.index(),
				child.index(),
				edge.length.map(f64::to_bits),
				edge.attributes.clone(),
			))
		})
		.collect::<Vec<_>>();
	edges.sort_unstable();
	edges
}

fn binary_node_by_name(tree: &BinaryTree, name: &str) -> Node {
	tree.nodes()
		.find(|&node| tree.name(node) == Some(name))
		.unwrap()
}

#[test]
fn deterministic_binary_shapes() -> Result<()> {
	let mut two_tip = TreeBuilder::with_root(NodeData::named("root"));
	let root = two_tip.root();
	add(&mut two_tip, root, "a", 1.0)?;
	add(&mut two_tip, root, "b", 2.0)?;

	let balanced = balanced_builder_4_leaves()?;

	let mut ladder = TreeBuilder::with_root(NodeData::named("root"));
	let root = ladder.root();
	add(&mut ladder, root, "d", 1.0)?;
	let first = add(&mut ladder, root, "first", 2.0)?;
	add(&mut ladder, first, "c", 3.0)?;
	let second = add(&mut ladder, first, "second", 4.0)?;
	add(&mut ladder, second, "a", 5.0)?;
	add(&mut ladder, second, "b", 6.0)?;

	for (builder, leaves, nodes) in
		[(two_tip, 2, 3), (balanced, 4, 7), (ladder, 4, 7)]
	{
		builder.validate()?;
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
fn node_and_edge_mutations() -> Result<()> {
	let mut tree = TreeBuilder::with_root(NodeData::named("root"));
	let root = tree.root();
	let left = add(&mut tree, root, "left", 1.0)?;
	let right = add(&mut tree, root, "right", 2.0)?;
	let child = add(&mut tree, left, "child", 3.0)?;

	tree.node_mut(child).attributes = "changed_node".into();
	tree.edge_mut(child).attributes = "changed_edge".into();
	assert_eq!(tree.node(child).attributes, "changed_node");
	assert_eq!(tree.edge(child).attributes, "changed_edge");

	tree.replace_parent(child, right)?;
	assert_eq!(tree.parent_of(child), Some(right));
	assert!(!tree.children_of(left).contains(&child));
	assert!(tree.children_of(right).contains(&child));

	let old = tree.replace_edge(child, edge_data(4.0, "replacement"))?;
	assert_eq!(old.length, Some(3.0));
	assert_eq!(old.attributes, "changed_edge");

	let removed = tree.remove_edge(right, child)?;
	assert_eq!(removed.length, Some(4.0));
	assert_eq!(tree.edge(child), &EdgeData::default());
	tree.replace_edge(child, edge_data(5.0, "detached"))
		.unwrap();
	assert_eq!(tree.edge(child), &edge_data(5.0, "detached"));
	assert!(tree.validate().is_err());
	tree.add_edge(left, child, removed)?;
	tree.validate()?;
	assert_eq!(tree.parent_of(child), Some(left));
	assert!(tree
		.add_edge(root, child, edge_data(1.0, "duplicate"))
		.is_err());
	assert!(tree.replace_parent(left, child).is_err());

	Ok(())
}

#[test]
fn rerooting_preserves_topology_and_data() -> Result<()> {
	let mut tree = balanced_builder_4_leaves()?;
	let original_root = tree.root();
	let new_root = tree
		.nodes()
		.find(|&node| tree.node(node).name == "left")
		.unwrap();
	let original_edges = canonical_edges(&tree);
	let connection = tree.edge(new_root).clone();

	tree.set_root(new_root)?;
	assert_eq!(tree.root(), new_root);
	assert_eq!(tree.parent_of(original_root), Some(new_root));
	assert_eq!(tree.edge(original_root), &connection);
	tree.validate()?;

	tree.set_root(original_root)?;
	assert_eq!(tree.root(), original_root);
	assert_eq!(canonical_edges(&tree), original_edges);
	assert!(tree.is_binary());
	tree.clone().into_binary()?;

	Ok(())
}

#[test]
fn binary_sealing_preserves_node_and_edge_data() -> Result<()> {
	let tree = balanced_builder_4_leaves()?;
	let expected_root_name = tree.node(tree.root()).name.clone();
	let expected = tree
		.nodes()
		.map(|node| {
			(
				tree.node(node).name.clone(),
				tree.node(node).attributes.clone(),
				tree.edge(node).clone(),
			)
		})
		.collect::<Vec<_>>();
	let binary = tree.into_binary()?;

	assert_eq!(
		binary.name(binary.root().into()),
		Some(expected_root_name.as_str())
	);
	for (name, node_attributes, edge) in expected {
		let node = binary_node_by_name(&binary, &name);
		assert_eq!(
			binary.node_metadata(node),
			Some(node_attributes.as_str())
		);
		assert_eq!(binary.edge_length(node), edge.length);
		assert_eq!(
			binary.edge_metadata(node).unwrap_or(""),
			edge.attributes.as_str()
		);
	}

	Ok(())
}

#[test]
fn multifurcating_and_hybrid_edges() -> Result<()> {
	let mut tree = TreeBuilder::with_root(NodeData::named("root"));
	let root = tree.root();
	let left = add(&mut tree, root, "left", 1.0)?;
	let right = add(&mut tree, root, "right", 2.0)?;
	let extra = add(&mut tree, root, "extra", 3.0)?;
	let left_leaf = add(&mut tree, left, "left_leaf", 4.0)?;
	let right_leaf = add(&mut tree, right, "right_leaf", 5.0)?;

	assert!(!tree.is_binary());
	assert!(tree.clone().into_binary().is_err());
	assert!(tree.add_hybrid_edge(right_leaf, right).is_err());
	tree.add_hybrid_edge(left, right_leaf)?;
	assert!(tree.add_hybrid_edge(left, right_leaf).is_err());
	assert!(tree.set_root(left).is_err());
	tree.remove_hybrid_edge(left, right_leaf)?;
	tree.add_hybrid_edge(left, right_leaf)?;
	tree.validate()?;

	assert_eq!(tree.num_nodes(), 6);
	assert_eq!(tree.parent_of(right_leaf), Some(right));
	assert_eq!(tree.parent_of(left_leaf), Some(left));
	assert_eq!(tree.node(right_leaf).name, "right_leaf");
	assert_eq!(tree.node(right_leaf).attributes, "node_right_leaf");
	assert_eq!(tree.edge(right_leaf), &edge_data(5.0, "edge_right_leaf"));
	assert_eq!(tree.children_of(root).len(), 3);
	assert!(tree.children_of(extra).is_empty());
	assert_eq!(
		tree.hybrid_edges().collect::<Vec<_>>(),
		vec![(left, right_leaf)]
	);

	Ok(())
}

#[test]
fn rejected_and_disconnected_edits() -> Result<()> {
	let mut tree = balanced_builder_4_leaves()?;
	let root = tree.root();
	let left = tree
		.nodes()
		.find(|&node| tree.node(node).name == "left")
		.unwrap();
	let leaf = tree
		.nodes()
		.find(|&node| tree.node(node).name == "a")
		.unwrap();

	assert!(tree.replace_parent(left, leaf).is_err());
	assert!(tree.add_hybrid_edge(leaf, root).is_err());
	assert!(tree.add_hybrid_edge(root, left).is_err());

	let edge = tree.remove_edge(left, leaf)?;
	assert!(tree.validate().is_err());
	assert!(tree.clone().into_binary().is_err());
	tree.add_edge(left, leaf, edge)?;
	tree.validate()?;

	Ok(())
}

fn arbitrary_builder(
	u: &mut Unstructured<'_>,
	num_leaves: u32,
) -> arbitrary::Result<TreeBuilder> {
	let mut tree = TreeBuilder::with_root(node_data("0", "node_0"));
	let mut leaves = vec![tree.root()];

	while leaves.len() < num_leaves as usize {
		let index = u.int_in_range(0..=leaves.len() - 1)?;
		let parent = leaves.swap_remove(index);
		for _ in 0..2 {
			let id = tree.num_nodes();
			let child = tree
				.add_node(
					parent,
					node_data(
						id.to_string(),
						format!("node_{id}"),
					),
					edge_data(
						f64::from(id) / 10.0,
						format!("edge_{id}"),
					),
				)
				.unwrap();
			leaves.push(child);
		}
	}

	Ok(tree)
}

#[test]
fn random_binary_builder_roundtrips() {
	arbtest(|u: &mut Unstructured<'_>| {
		let num_leaves = u.int_in_range(2_u32..=128)?;
		let mut builder = arbitrary_builder(u, num_leaves)?;
		builder.validate().unwrap();
		assert!(builder.is_binary());

		let original_edges = canonical_edges(&builder);
		let original_root = builder.root();
		let new_root_index =
			u.int_in_range(1..=builder.num_nodes() - 1)?;
		let new_root =
			builder.nodes().nth(new_root_index as usize).unwrap();
		builder.set_root(new_root).unwrap();
		builder.set_root(original_root).unwrap();
		assert_eq!(canonical_edges(&builder), original_edges);

		let binary = builder.clone().into_binary().unwrap();
		assert_eq!(binary.num_leaves(), num_leaves);
		assert_eq!(binary.num_nodes(), builder.num_nodes());
		assert_eq!(
			binary.preorder().count(),
			builder.num_nodes() as usize
		);
		assert_eq!(
			binary.postorder().count(),
			builder.num_nodes() as usize
		);

		for source in builder.nodes() {
			let name = &builder.node(source).name;
			let target = binary_node_by_name(&binary, name);
			assert_eq!(
				binary.node_metadata(target),
				Some(builder.node(source).attributes.as_str())
			);
			let edge = builder.edge(source);
			assert_eq!(binary.edge_length(target), edge.length);
			assert_eq!(
				binary.edge_metadata(target).unwrap_or(""),
				edge.attributes.as_str()
			);
			if source == builder.root() {
				continue;
			}
			let source_parent = builder.parent_of(source).unwrap();
			let target_parent = binary.parent_of(target).unwrap();
			assert_eq!(
				binary.name(target_parent.into()),
				Some(builder.node(source_parent).name.as_str())
			);
		}

		Ok(())
	});
}

#[test]
fn large_ladder_traversal() -> Result<()> {
	let num_leaves = 20_000_u32;
	let mut tree = TreeBuilder::with_root(NodeData::named("root"));
	let mut current = tree.root();
	for index in 1..num_leaves - 1 {
		add(&mut tree, current, &format!("leaf_{index}"), 1.0)?;
		current = add(
			&mut tree,
			current,
			&format!("internal_{index}"),
			1.0,
		)?;
	}
	add(&mut tree, current, "last_left", 1.0)?;
	add(&mut tree, current, "last_right", 1.0)?;

	let tree = tree.into_binary()?;
	assert_eq!(tree.num_leaves(), num_leaves);
	assert_eq!(tree.preorder().count(), tree.num_nodes() as usize);
	assert_eq!(tree.postorder().count(), tree.num_nodes() as usize);

	Ok(())
}
