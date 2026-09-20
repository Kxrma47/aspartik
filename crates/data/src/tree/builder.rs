use anyhow::{Result, anyhow, bail, ensure};
use buffer::Buffer;
use picoarrow::array::{ArrayUtf8, Nullable};
use smallvec::SmallVec;

use std::{collections::VecDeque, mem};

use super::{BinaryTree, Node, ROOT_PARENT};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeData {
	pub name: String,
	pub attributes: String,
}

impl NodeData {
	pub const fn new(name: String, attributes: String) -> NodeData {
		NodeData { name, attributes }
	}

	pub fn named(name: impl AsRef<str>) -> NodeData {
		NodeData::new(name.as_ref().to_owned(), String::new())
	}

	pub const fn unnamed() -> NodeData {
		NodeData::new(String::new(), String::new())
	}
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EdgeData {
	pub length: Option<f64>,
	pub attributes: String,
}

impl EdgeData {
	pub const fn new(length: Option<f64>, attributes: String) -> Self {
		EdgeData { length, attributes }
	}

	pub const fn from_distance(length: f64) -> EdgeData {
		EdgeData::new(Some(length), String::new())
	}

	pub const fn without_distance() -> EdgeData {
		EdgeData::new(None, String::new())
	}
}

#[derive(Debug, Clone)]
pub struct TreeBuilder {
	pub(super) children: Vec<SmallVec<[Node; 2]>>,
	pub(super) parents: Vec<Node>,
	pub(super) edges: Vec<EdgeData>,
	pub(super) nodes: Vec<NodeData>,
	pub(super) root: Node,
}

impl Default for TreeBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl TreeBuilder {
	pub fn new() -> Self {
		Self::with_root(NodeData::unnamed())
	}

	pub fn with_root(data: NodeData) -> Self {
		Self {
			children: vec![SmallVec::new()],
			parents: vec![Node(ROOT_PARENT)],
			edges: vec![EdgeData::default()],
			nodes: vec![data],
			root: Node(0),
		}
	}

	pub fn num_nodes(&self) -> u32 {
		self.nodes.len() as u32
	}

	pub fn root(&self) -> Node {
		self.root
	}

	pub fn nodes(&self) -> impl DoubleEndedIterator<Item = Node> + use<> {
		(0..self.num_nodes()).map(Node)
	}

	pub fn contains(&self, node: Node) -> bool {
		node.usize() < self.nodes.len()
	}

	pub fn is_leaf(&self, node: Node) -> bool {
		self.children_of(node).is_empty()
	}

	pub fn is_binary(&self) -> bool {
		self.children
			.iter()
			.all(|children| matches!(children.len(), 0 | 2))
	}

	pub fn children_of(&self, node: Node) -> &[Node] {
		self.children[node.usize()].as_slice()
	}

	pub fn parent_of(&self, node: Node) -> Option<Node> {
		self.parents
			.get(node.usize())
			.copied()
			.filter(|parent| parent.u32() != ROOT_PARENT)
	}

	pub fn edge(&self, node: Node) -> &EdgeData {
		&self.edges[node.usize()]
	}

	pub fn edge_mut(&mut self, node: Node) -> &mut EdgeData {
		&mut self.edges[node.usize()]
	}

	pub fn node(&self, node: Node) -> &NodeData {
		&self.nodes[node.usize()]
	}

	pub fn node_mut(&mut self, node: Node) -> &mut NodeData {
		&mut self.nodes[node.usize()]
	}

	pub fn hybrid_edges(
		&self,
	) -> impl Iterator<Item = (Node, Node)> + use<'_> {
		self.children.iter().enumerate().flat_map(
			move |(i, children)| {
				let parent = Node(i as u32);
				children.iter()
					.copied()
					.filter(move |&child| {
						self.parents[child.usize()]
							!= parent
					})
					.map(move |child| (parent, child))
			},
		)
	}

	pub fn add_node(
		&mut self,
		parent: Node,
		data: NodeData,
		edge: EdgeData,
	) -> Result<Node> {
		self.ensure_valid_node(parent)?;
		let index = u32::try_from(self.nodes.len()).map_err(|_| {
			anyhow!("The number of nodes does not fit in u32")
		})?;
		let node = Node(index);
		self.nodes.push(data);
		self.children.push(SmallVec::new());
		self.parents.push(parent);
		self.edges.push(edge);
		self.children[parent.usize()].push(node);
		Ok(node)
	}

	pub fn add_edge(
		&mut self,
		parent: Node,
		child: Node,
		edge: EdgeData,
	) -> Result<()> {
		self.ensure_valid_node(parent)?;
		self.ensure_valid_node(child)?;
		ensure!(child != self.root, "The root cannot have a parent");
		ensure!(
			self.parents[child.usize()].u32() == ROOT_PARENT,
			"Node {} already has a canonical parent",
			child.u32()
		);
		ensure!(
			!self.reaches(child, parent),
			"Adding the edge would create a cycle"
		);

		self.parents[child.usize()] = parent;
		self.edges[child.usize()] = edge;
		self.children[parent.usize()].push(child);
		Ok(())
	}

	pub fn remove_edge(
		&mut self,
		parent: Node,
		child: Node,
	) -> Result<EdgeData> {
		self.ensure_valid_node(parent)?;
		self.ensure_valid_node(child)?;
		ensure!(
			self.parents[child.usize()] == parent,
			"Node {} is not a canonical child of node {}",
			child.u32(),
			parent.u32()
		);

		let index = self.children[parent.usize()]
			.iter()
			.position(|&node| node == child)
			.ok_or_else(|| {
				anyhow!(
					"The parent-child relation is inconsistent"
				)
			})?;
		self.children[parent.usize()].remove(index);
		self.parents[child.usize()] = Node(ROOT_PARENT);
		Ok(mem::take(&mut self.edges[child.usize()]))
	}

	pub fn replace_parent(
		&mut self,
		child: Node,
		new_parent: Node,
	) -> Result<()> {
		self.ensure_valid_node(new_parent)?;
		self.ensure_valid_node(child)?;
		ensure!(child != self.root, "The root cannot have a parent");
		let Some(old_parent) = self.parent_of(child) else {
			bail!("Node {} has no parent", child.u32())
		};
		if old_parent == new_parent {
			return Ok(());
		}
		ensure!(
			!self.reaches(child, new_parent),
			"Changing the parent would create a cycle"
		);

		let Some(index) = self.children[old_parent.usize()]
			.iter()
			.position(|&node| node == child)
		else {
			bail!("The parent-child relation is inconsistent")
		};

		self.children[old_parent.usize()].remove(index);
		self.children[new_parent.usize()].push(child);
		self.parents[child.usize()] = new_parent;
		Ok(())
	}

	pub fn replace_edge(
		&mut self,
		child: Node,
		edge: EdgeData,
	) -> Result<EdgeData> {
		self.ensure_valid_node(child)?;
		ensure!(child != self.root, "The root has no incoming edge");
		let current = &mut self.edges[child.usize()];
		Ok(mem::replace(current, edge))
	}

	pub fn add_hybrid_edge(
		&mut self,
		parent: Node,
		child: Node,
	) -> Result<()> {
		self.ensure_valid_node(parent)?;
		self.ensure_valid_node(child)?;
		ensure!(parent != child, "A node cannot be its own parent");
		ensure!(
			self.parents[child.usize()] != parent,
			"The edge is already the canonical parent relation"
		);
		ensure!(
			!self.children[parent.usize()].contains(&child),
			"The hybrid edge already exists"
		);
		ensure!(
			!self.reaches(child, parent),
			"Adding the hybrid edge would create a cycle"
		);

		self.children[parent.usize()].push(child);
		Ok(())
	}

	pub fn remove_hybrid_edge(
		&mut self,
		parent: Node,
		child: Node,
	) -> Result<()> {
		self.ensure_valid_node(parent)?;
		self.ensure_valid_node(child)?;
		ensure!(
			self.parents[child.usize()] != parent,
			"The edge is the canonical parent relation"
		);
		let index = self.children[parent.usize()]
			.iter()
			.position(|&entry| entry == child)
			.ok_or_else(|| {
				anyhow!("The hybrid edge does not exist")
			})?;
		self.children[parent.usize()].remove(index);
		Ok(())
	}

	pub fn set_root(&mut self, node: Node) -> Result<()> {
		self.ensure_valid_node(node)?;
		if node == self.root {
			return Ok(());
		}
		ensure!(
			self.hybrid_edges().next().is_none(),
			"Rerooting a tree with hybrid edges is not supported"
		);
		self.validate()?;

		let mut path = Vec::new();
		let mut current = node;
		while current != self.root {
			let parent =
				self.parent_of(current).ok_or_else(|| {
					anyhow!("The new root is disconnected")
				})?;
			let edge = self.edges[current.usize()].clone();
			path.push((current, parent, edge));
			current = parent;
		}

		for (child, parent, edge) in path {
			let index = self.children[parent.usize()]
				.iter()
				.position(|&entry| entry == child)
				.ok_or_else(|| {
					anyhow!(
						"The parent-child relation is inconsistent"
					)
				})?;
			self.children[parent.usize()].remove(index);
			self.children[child.usize()].push(parent);
			self.parents[parent.usize()] = child;
			self.edges[parent.usize()] = edge;
		}

		self.parents[node.usize()] = Node(ROOT_PARENT);
		self.edges[node.usize()] = EdgeData::default();
		self.root = node;
		self.validate()
	}

	pub fn validate(&self) -> Result<()> {
		let num_nodes = self.nodes.len();
		ensure!(num_nodes > 0, "Expected at least one node");
		ensure!(
			self.root.usize() < num_nodes,
			"The root is out of range"
		);
		ensure!(
			self.children.len() == num_nodes
				&& self.parents.len() == num_nodes
				&& self.edges.len() == num_nodes,
			"Tree storage lengths are inconsistent"
		);

		let mut seen = vec![false; num_nodes];
		for (parent, children) in self.children.iter().enumerate() {
			let mut unique = SmallVec::<[Node; 2]>::new();
			for &child in children {
				ensure!(
					child.usize() < num_nodes,
					"A child node is out of range"
				);
				ensure!(
					child != self.root,
					"The root appears as a child"
				);
				ensure!(
					!unique.contains(&child),
					"Node {} appears more than once under the same parent",
					child.u32()
				);
				unique.push(child);
				if self.parents[child.usize()]
					== Node(parent as u32)
				{
					ensure!(
						!seen[child.usize()],
						"Node {} appears as a canonical child more than once",
						child.u32()
					);
					seen[child.usize()] = true;
				}
			}
		}

		for node in self.nodes() {
			if node == self.root {
				ensure!(
					self.parents[node.usize()].u32()
						== ROOT_PARENT,
					"The root has a parent"
				);
			} else {
				ensure!(
					seen[node.usize()],
					"Node {} is disconnected",
					node.u32()
				);
				ensure!(
					self.parents[node.usize()].u32()
						!= ROOT_PARENT,
					"Node {} has no canonical parent",
					node.u32()
				);
			}
		}

		let mut reachable = vec![false; num_nodes];
		let mut queue = VecDeque::from([self.root]);
		while let Some(node) = queue.pop_front() {
			ensure!(
				!reachable[node.usize()],
				"The canonical tree contains a cycle"
			);
			reachable[node.usize()] = true;
			queue.extend(self.children[node.usize()]
				.iter()
				.copied()
				.filter(|child| {
					self.parents[child.usize()] == node
				}));
		}
		ensure!(
			reachable.into_iter().all(|value| value),
			"Not all nodes are reachable from the root"
		);

		let mut indegrees = vec![0_u32; num_nodes];
		for children in &self.children {
			for child in children {
				indegrees[child.usize()] += 1;
			}
		}
		let mut queue = indegrees
			.iter()
			.enumerate()
			.filter_map(|(node, &degree)| {
				(degree == 0).then_some(Node(node as u32))
			})
			.collect::<VecDeque<_>>();
		let mut visited = 0;
		while let Some(node) = queue.pop_front() {
			visited += 1;
			for &child in &self.children[node.usize()] {
				indegrees[child.usize()] -= 1;
				if indegrees[child.usize()] == 0 {
					queue.push_back(child);
				}
			}
		}
		ensure!(
			visited == num_nodes,
			"The tree network contains a cycle"
		);
		Ok(())
	}

	pub fn into_binary(self) -> Result<BinaryTree> {
		BinaryTree::try_from(self)
	}

	fn ensure_valid_node(&self, node: Node) -> Result<()> {
		ensure!(self.contains(node), "Node {} is out of range", node.0);
		Ok(())
	}

	fn reaches(&self, start: Node, target: Node) -> bool {
		let mut seen = vec![false; self.nodes.len()];
		let mut stack = vec![start];
		while let Some(node) = stack.pop() {
			if node == target {
				return true;
			}
			if seen[node.usize()] {
				continue;
			}
			seen[node.usize()] = true;
			stack.extend(self.children[node.usize()]
				.iter()
				.copied());
		}
		false
	}
}

impl TryFrom<TreeBuilder> for BinaryTree {
	type Error = anyhow::Error;

	fn try_from(builder: TreeBuilder) -> Result<Self> {
		builder.validate()?;
		ensure!(builder.is_binary(), "The tree is not binary");

		let leaves = builder
			.nodes()
			.filter(|&node| builder.is_leaf(node))
			.collect::<Vec<_>>();
		ensure!(
			leaves.len() >= 2,
			"Expected at least two leaves, got {}",
			leaves.len()
		);
		let internals = builder
			.nodes()
			.filter(|&node| !builder.is_leaf(node))
			.collect::<Vec<_>>();
		let num_leaves = u32::try_from(leaves.len())?;
		let mut order = leaves;
		order.extend(internals);
		let mut mapping = vec![0_u32; builder.nodes.len()];
		for (new, old) in order.iter().copied().enumerate() {
			mapping[old.usize()] = u32::try_from(new)?;
		}

		let mut children = Vec::with_capacity(builder.nodes.len() - 1);
		for &old in &order[num_leaves as usize..] {
			children.extend(builder.children[old.usize()]
				.iter()
				.map(|child| mapping[child.usize()]));
		}
		let root = mapping[builder.root.usize()];
		let mut edge_lengths = vec![0.0; builder.nodes.len() - 1];
		let mut edge_attributes = vec![None; builder.nodes.len() - 1];
		for &old in &order {
			let new = mapping[old.usize()];
			if new == root {
				continue;
			}
			let index = (new - u32::from(new > root)) as usize;
			let edge = &builder.edges[old.usize()];
			edge_lengths[index] = edge.length.ok_or_else(|| {
				anyhow!("Node {} has no edge length", old.u32())
			})?;
			edge_attributes[index] = nonempty(&edge.attributes);
		}

		let mut names = ArrayUtf8::<Nullable>::new();
		let mut node_attributes = ArrayUtf8::<Nullable>::new();
		for &old in &order {
			names.push(nonempty(&builder.nodes[old.usize()].name))?;
			node_attributes.push(nonempty(
				&builder.nodes[old.usize()].attributes,
			))?;
		}
		let mut edge_metadata = ArrayUtf8::<Nullable>::new();
		for attributes in edge_attributes {
			edge_metadata.push(attributes)?;
		}

		Self::canonical(
			num_leaves,
			root,
			Buffer::from_slice(&children),
			Buffer::from_slice(&edge_lengths),
			names,
			node_attributes,
			edge_metadata,
		)
	}
}

fn nonempty(value: &str) -> Option<&str> {
	(!value.is_empty()).then_some(value)
}
