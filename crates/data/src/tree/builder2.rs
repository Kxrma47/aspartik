use smallvec::SmallVec;

use super::Node;

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
}
