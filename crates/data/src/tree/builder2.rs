use anyhow::{Result, anyhow};
use picoarrow::array::{Array, ArrayUtf8, Nullable};
use smallvec::SmallVec;

use super::{BinaryTree, Node};
use crate::TaxonSet;
use buffer::Buffer;

#[derive(Debug, Clone, Default)]
pub struct TreeBuilder2 {
	parents: Vec<Node>,
	children: Vec<SmallVec<[Node; 2]>>,
	node_names: Vec<String>,
	node_attributes: Vec<String>,
	edge_lengths: Vec<f64>,
	edge_attributes: Vec<String>,
	root: Option<Node>,
}

impl TreeBuilder2 {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn add_dummy_node(&mut self) -> Node {
		let idx = self.parents.len() as u32;
		self.parents.push(Node::TOMBSTONE);
		self.children.push(SmallVec::new());
		self.node_names.push(String::new());
		self.node_attributes.push(String::new());
		self.edge_lengths.push(f64::NAN);
		self.edge_attributes.push(String::new());

		Node(idx)
	}

	pub fn add_node(
		&mut self,
		name: &str,
		edge_length: f64,
		parent: Node,
	) -> Node {
		let idx = self.parents.len() as u32;
		let new = Node(idx);

		self.parents.push(parent);
		self.children.push(SmallVec::new());
		self.node_names.push(name.to_owned());
		self.node_attributes.push(String::new());
		self.edge_lengths.push(edge_length);
		self.edge_attributes.push(String::new());

		self.children[parent.usize()].push(new);

		new
	}

	/// Delete all nodes leaving the buffers for reuse
	pub fn clear(&mut self) {
		self.parents.clear();
		self.children.clear();
		self.node_names.clear();
		self.node_attributes.clear();
		self.edge_lengths.clear();
		self.edge_attributes.clear();
		self.root = None;
	}

	pub fn set_root(&mut self, node: Node) {
		self.root = Some(node);
	}

	pub fn num_nodes(&self) -> usize {
		self.parents.len()
	}

	// Node setters
	pub fn set_node_parent(&mut self, node: Node, parent: Node) {
		self.parents[node.usize()] = parent;
	}

	pub fn set_node_children(&mut self, node: Node, children: &[Node]) {
		self.children[node.usize()] = SmallVec::from_slice(children);
	}

	pub fn set_node_name(&mut self, node: Node, name: &str) {
		self.node_names[node.usize()].replace_range(.., name);
	}

	pub fn set_node_attribute(&mut self, node: Node, attr: &str) {
		self.node_attributes[node.usize()].replace_range(.., attr);
	}

	pub fn set_edge_length(&mut self, node: Node, length: f64) {
		self.edge_lengths[node.usize()] = length;
	}

	pub fn set_edge_attribute(&mut self, node: Node, attr: &str) {
		self.edge_attributes[node.usize()].replace_range(.., attr);
	}

	// Node getters
	pub fn parent_of(&self, node: Node) -> Node {
		self.parents[node.usize()]
	}

	pub fn children_of(&self, node: Node) -> &[Node] {
		&self.children[node.usize()]
	}

	pub fn name(&self, node: Node) -> &str {
		&self.node_names[node.usize()]
	}

	pub fn node_attribute(&self, node: Node) -> &str {
		&self.node_attributes[node.usize()]
	}

	pub fn edge_length(&self, node: Node) -> Option<f64> {
		let length = self.edge_lengths[node.usize()];
		if length.is_nan() { None } else { Some(length) }
	}

	pub fn edge_attribute(&self, node: Node) -> &str {
		&self.edge_attributes[node.usize()]
	}

	pub fn root(&self) -> Option<Node> {
		self.root
	}

	pub fn is_binary(&self) -> bool {
		self.children
			.iter()
			.all(|children| matches!(children.len(), 0 | 2))
	}

	pub fn to_binary(&self, taxon_set: TaxonSet) -> Result<BinaryTree> {
		to_binary(self, taxon_set)
	}
}

fn to_binary(
	builder: &TreeBuilder2,
	taxon_set: TaxonSet,
) -> Result<BinaryTree> {
	let root = builder.root().ok_or_else(|| anyhow!("tree has no root"))?;
	let num_nodes = builder.num_nodes() as u32;
	let num_leaves = taxon_set.len() as u32;

	let unmapped = u32::MAX;
	let mut mapping = vec![unmapped; num_nodes as usize];
	for node in (0..num_nodes).map(Node) {
		if builder.children_of(node).is_empty() {
			let name = builder.name(node);
			let taxon = taxon_set
				.iter()
				.position(|taxon| taxon == name)
				.ok_or_else(|| {
					anyhow!("unknown leaf name: {name:?}")
				})?;
			mapping[node.usize()] = taxon as u32;
		}
	}

	let mut children =
		Buffer::repeat(0_u32, num_nodes.checked_sub(1).unwrap());
	let mut lengths = Buffer::repeat(f64::NAN, num_nodes);
	let mut internal_names = ArrayUtf8::<Nullable>::new();
	let mut next_internal = num_leaves;
	let mut internals =
		Vec::with_capacity(num_nodes as usize - num_leaves as usize);

	let mut stack = vec![(root, false)];
	while let Some((node, expanded)) = stack.pop() {
		if !expanded {
			if !builder.children_of(node).is_empty() {
				stack.push((node, true));
				stack.extend(builder
					.children_of(node)
					.iter()
					.rev()
					.map(|&child| (child, false)));
			}
			continue;
		}

		if node != root {
			mapping[node.usize()] = next_internal;
			next_internal += 1;
			internals.push(node);
			internal_names.push(Some(builder.name(node)))?;
		}
	}

	let new_root = num_nodes - 1;
	mapping[root.usize()] = new_root;
	internals.push(root);
	for node in (0..num_nodes).map(Node) {
		let new = mapping[node.usize()];
		if new != new_root {
			lengths[new as usize] =
				builder.edge_length(node).unwrap();
		}
	}
	for (offset, &old) in internals.iter().enumerate() {
		for (slot, child) in builder.children_of(old).iter().enumerate()
		{
			children[offset * 2 + slot] = mapping[child.usize()];
		}
	}

	BinaryTree::from_children(
		new_root,
		children,
		lengths,
		taxon_set,
		internal_names,
	)
}
