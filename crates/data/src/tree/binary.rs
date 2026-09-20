use anyhow::{Result, anyhow, ensure};
use picoarrow::array::{Array, ArrayUtf8, Nullable};
use rand::{Rng, seq::SliceRandom};

use std::{
	cmp::{Reverse, max, min},
	collections::BinaryHeap,
};

use super::{Internal, Leaf, Node, ROOT_PARENT};
use buffer::Buffer;

#[derive(Debug)]
pub struct BinaryTree {
	num_leaves: u32,
	root: u32,
	children: Buffer<u32>,
	parents: Buffer<u32>,
	edge_lengths: Buffer<f64>,
	node_names: ArrayUtf8<Nullable>,
	node_metadata: ArrayUtf8<Nullable>,
	edge_metadata: ArrayUtf8<Nullable>,
}

impl BinaryTree {
	fn validate_num_leaves(num_leaves: u32) -> Result<()> {
		ensure!(
			num_leaves >= 2,
			"Expected at least two leaves, got {num_leaves}"
		);
		ensure!(
			num_leaves <= u32::MAX / 2,
			"Expected at most {} leaves, got {num_leaves}",
			u32::MAX / 2
		);
		Ok(())
	}

	pub fn random<R: Rng>(num_leaves: u32, rng: &mut R) -> Result<Self> {
		ensure!(
			num_leaves >= 2,
			"Expected at least two leaves, got {num_leaves}"
		);

		let num_nodes = num_leaves
			.checked_mul(2)
			.and_then(|value| value.checked_sub(1))
			.ok_or_else(|| {
				anyhow!(
					"The number of nodes does not fit in u32"
				)
			})?;
		let num_internals = num_leaves - 1;
		let num_nodes_usize = usize::try_from(num_nodes)?;
		let num_edges = num_nodes - 1;
		let num_edges_usize = num_nodes_usize - 1;

		let internals = num_leaves..num_nodes;
		let mut parents = Buffer::repeat(ROOT_PARENT, num_nodes);
		for (slot, parent) in parents[..num_edges_usize]
			.iter_mut()
			.zip(internals.clone().chain(internals))
		{
			*slot = parent;
		}
		let root = parents[num_edges_usize - 1];
		parents[..num_edges_usize - 1].shuffle(rng);

		let mut children = Buffer::repeat(ROOT_PARENT, num_edges);
		let mut remaining = vec![2; num_internals as usize];
		*remaining.last_mut().unwrap() = 1;
		let mut unused =
			BinaryHeap::from_iter((0..num_leaves).map(Reverse));
		let mut removed = Vec::with_capacity(num_edges_usize);

		for step in 0..num_edges_usize - 1 {
			let parent = parents[step];
			let child = unused.pop().unwrap().0;
			removed.push(child);
			let offset = ((parent - num_leaves) * 2) as usize;
			if children[offset] == ROOT_PARENT {
				children[offset] = child;
			} else {
				children[offset + 1] = child;
			}

			let index = (parent - num_leaves) as usize;
			remaining[index] -= 1;
			if remaining[index] == 0 {
				unused.push(Reverse(parent));
			}
		}

		let child = unused.pop().unwrap().0;
		removed.push(child);
		children[(root - num_leaves) as usize * 2 + 1] = child;
		for index in 0..num_edges_usize {
			while removed[index] != index as u32 {
				let target = removed[index] as usize;
				parents.swap(index, target);
				removed.swap(index, target);
			}
		}

		let mut node_names = ArrayUtf8::<Nullable>::new();
		let mut node_metadata = ArrayUtf8::<Nullable>::new();
		for _ in 0..num_nodes_usize {
			node_names.push(None)?;
			node_metadata.push(None)?;
		}
		let mut edge_metadata = ArrayUtf8::<Nullable>::new();
		for _ in 0..num_edges_usize {
			edge_metadata.push(None)?;
		}
		let edge_lengths = Buffer::repeat(0.0, num_edges);

		Ok(Self {
			num_leaves,
			root,
			children,
			parents,
			edge_lengths,
			node_names,
			node_metadata,
			edge_metadata,
		})
	}

	#[allow(clippy::too_many_arguments)]
	pub fn new(
		num_leaves: u32,
		root: u32,
		children: Buffer<u32>,
		parents: Buffer<u32>,
		edge_lengths: Buffer<f64>,
		node_names: ArrayUtf8<Nullable>,
		node_metadata: ArrayUtf8<Nullable>,
		edge_metadata: ArrayUtf8<Nullable>,
	) -> Result<Self> {
		let mut tree = Self {
			num_leaves,
			root,
			children,
			parents,
			edge_lengths,
			node_names,
			node_metadata,
			edge_metadata,
		};
		tree.validate()?;
		tree.node_names.shrink_to_fit();
		tree.node_metadata.shrink_to_fit();
		tree.edge_metadata.shrink_to_fit();
		Ok(tree)
	}

	pub fn from_children(
		num_leaves: u32,
		root: u32,
		children: Buffer<u32>,
		edge_lengths: Buffer<f64>,
		node_names: ArrayUtf8<Nullable>,
		node_metadata: ArrayUtf8<Nullable>,
		edge_metadata: ArrayUtf8<Nullable>,
	) -> Result<Self> {
		ensure!(
			num_leaves >= 2,
			"Expected at least two leaves, got {num_leaves}"
		);
		let num_nodes = num_leaves
			.checked_mul(2)
			.and_then(|value| value.checked_sub(1))
			.ok_or_else(|| {
				anyhow!(
					"The number of nodes does not fit in u32"
				)
			})?;
		ensure!(
			children.len() == num_nodes - 1,
			"Expected {} child entries, got {}",
			num_nodes - 1,
			children.len()
		);
		let mut parents = Buffer::repeat(ROOT_PARENT, num_nodes);
		for (offset, pair) in
			children.as_chunks::<2>().0.iter().enumerate()
		{
			let parent = num_leaves
				.checked_add(u32::try_from(offset)?)
				.ok_or_else(|| {
					anyhow!(
						"The number of nodes does not fit in u32"
					)
				})?;
			for &child in pair {
				let slot = parents
					.get_mut(child as usize)
					.ok_or_else(|| {
						anyhow!(
							"Child {child} of node {parent} is out of range"
						)
					})?;
				*slot = parent;
			}
		}
		let tree = Self::new(
			num_leaves,
			root,
			children,
			parents,
			edge_lengths,
			node_names,
			node_metadata,
			edge_metadata,
		)?;
		let mut order = (0..num_leaves).collect::<Vec<_>>();
		order.extend(tree.postorder().filter_map(|node| {
			tree.is_internal(node).then_some(node.u32())
		}));
		if order.iter().copied().eq(0..num_nodes) {
			return Ok(tree);
		}

		let mut mapping = vec![0; num_nodes as usize];
		for (new, &old) in order.iter().enumerate() {
			mapping[old as usize] = u32::try_from(new)?;
		}
		let num_edges = num_nodes - 1;
		let mut children = Buffer::repeat(0, num_edges);
		let mut parents = Buffer::repeat(ROOT_PARENT, num_nodes);
		for (offset, &old) in
			order[num_leaves as usize..].iter().enumerate()
		{
			let parent = num_leaves + u32::try_from(offset)?;
			let [left, right] = tree.children_of(Internal(old));
			for (slot, child) in
				[left, right].into_iter().enumerate()
			{
				let child = mapping[child.usize()];
				children[offset * 2 + slot] = child;
				parents[child as usize] = parent;
			}
		}
		let mut lengths = Buffer::repeat(0.0, num_edges);
		let mut names = ArrayUtf8::<Nullable>::new();
		let mut node_metadata = ArrayUtf8::<Nullable>::new();
		let mut edge_metadata = ArrayUtf8::<Nullable>::new();
		for (new, &old) in order.iter().enumerate() {
			let old = Node(old);
			names.push(tree.name(old))?;
			node_metadata.push(tree.node_metadata(old))?;
			if new < num_edges as usize {
				lengths[new] = tree.edge_length(old).unwrap();
				edge_metadata.push(tree.edge_metadata(old))?;
			}
		}
		Self::new(
			num_leaves,
			num_nodes - 1,
			children,
			parents,
			lengths,
			names,
			node_metadata,
			edge_metadata,
		)
	}

	pub fn canonical(&self) -> Result<Self> {
		let mut leaves = Vec::with_capacity(self.num_leaves as usize);
		for leaf in self.leaves() {
			let name = self.name(leaf.into()).ok_or_else(|| {
				anyhow!("Leaf {} has no name", leaf.u32())
			})?;
			ensure!(
				!name.is_empty(),
				"Leaf {} has no name",
				leaf.u32()
			);
			leaves.push((name, leaf.u32()));
		}
		leaves.sort_unstable_by(|left, right| left.0.cmp(right.0));
		for pair in leaves.windows(2) {
			ensure!(
				pair[0].0 != pair[1].0,
				"Duplicate leaf name: {}",
				pair[0].0
			);
		}

		let num_leaves = self.num_leaves;
		let num_nodes = self.num_nodes();
		let num_edges = self.num_edges();
		let mut mapping = vec![0; num_nodes as usize];
		let mut order = Vec::with_capacity(num_nodes as usize);
		let mut minima = vec![0; num_nodes as usize];
		for (index, &(_, old)) in leaves.iter().enumerate() {
			minima[old as usize] = index as u32;
			order.push(old);
		}
		for node in self.postorder() {
			if let Some(internal) = self.as_internal(node) {
				let [left, right] = self.children_of(internal);
				minima[node.usize()] = minima[left.usize()]
					.min(minima[right.usize()]);
			}
		}
		let ordered_children = |internal| {
			let [left, right] = self.children_of(internal);
			if (self.is_internal(left), minima[left.usize()])
				< (
					self.is_internal(right),
					minima[right.usize()],
				) {
				[left, right]
			} else {
				[right, left]
			}
		};

		let mut stack = vec![(Node(self.root), false)];
		while let Some((node, visited)) = stack.pop() {
			let Some(internal) = self.as_internal(node) else {
				continue;
			};
			if visited {
				order.push(node.u32());
				continue;
			}
			let [left, right] = ordered_children(internal);
			stack.push((node, true));
			stack.push((right, false));
			stack.push((left, false));
		}
		for (index, &old) in order.iter().enumerate() {
			mapping[old as usize] = index as u32;
		}

		let mut children = Buffer::repeat(0, num_edges);
		let mut parents = Buffer::repeat(ROOT_PARENT, num_nodes);
		for (index, &old) in
			order[num_leaves as usize..].iter().enumerate()
		{
			let parent = num_leaves + index as u32;
			let [left, right] = ordered_children(Internal(old));
			for (slot, child) in
				[left, right].into_iter().enumerate()
			{
				let child = mapping[child.usize()];
				children[index * 2 + slot] = child;
				parents[child as usize] = parent;
			}
		}

		let mut lengths = Buffer::repeat(0.0, num_edges);
		let mut names = ArrayUtf8::<Nullable>::with_capacity(
			num_nodes as usize,
		);
		let mut node_metadata = ArrayUtf8::<Nullable>::with_capacity(
			num_nodes as usize,
		);
		let mut edge_metadata = ArrayUtf8::<Nullable>::with_capacity(
			num_edges as usize,
		);
		for (index, &old) in order.iter().enumerate() {
			let old = Node(old);
			names.push(self.name(old))?;
			node_metadata.push(self.node_metadata(old))?;
			if index < num_edges as usize {
				lengths[index] = self.edge_length(old).unwrap();
				edge_metadata.push(self.edge_metadata(old))?;
			}
		}
		Self::new(
			num_leaves,
			num_nodes - 1,
			children,
			parents,
			lengths,
			names,
			node_metadata,
			edge_metadata,
		)
	}

	pub fn validate(&self) -> Result<()> {
		let num_leaves = self.num_leaves;
		let root = self.root;
		Self::validate_num_leaves(num_leaves)?;
		let num_nodes = num_leaves * 2 - 1;
		let num_edges = num_nodes - 1;
		let num_nodes_usize = num_nodes as usize;
		let num_edges_usize = num_edges as usize;
		ensure!(
			(num_leaves..num_nodes).contains(&root),
			"Root node {root} is not an internal node"
		);
		ensure!(
			self.children.len() == num_edges,
			"Expected {num_edges} child entries, got {}",
			self.children.len()
		);
		ensure!(
			self.parents.len() == num_nodes,
			"Expected {num_nodes} parent entries, got {}",
			self.parents.len()
		);
		ensure!(
			self.edge_lengths.len() == num_edges,
			"Expected {num_edges} edge lengths, got {}",
			self.edge_lengths.len()
		);
		ensure!(
			self.node_names.len() == num_nodes_usize,
			"Expected {num_nodes} node names, got {}",
			self.node_names.len()
		);
		ensure!(
			self.node_metadata.len() == num_nodes_usize,
			"Expected {num_nodes} node metadata entries, got {}",
			self.node_metadata.len()
		);
		ensure!(
			self.edge_metadata.len() == num_edges_usize,
			"Expected {num_edges} edge metadata entries, got {}",
			self.edge_metadata.len()
		);

		let mut seen = vec![false; num_nodes_usize];
		for (offset, pair) in
			self.children.as_chunks::<2>().0.iter().enumerate()
		{
			let parent = num_leaves + u32::try_from(offset)?;
			for &child in pair {
				ensure!(
					child < num_nodes,
					"Child {child} of node {parent} is out of range"
				);
				ensure!(
					!seen[child as usize],
					"Node {child} appears as a child more than once"
				);
				ensure!(
					self.parents[child as usize] == parent,
					"Parent of node {child} is inconsistent"
				);
				seen[child as usize] = true;
			}
		}

		for node in 0..num_nodes {
			if node == root {
				ensure!(
					!seen[node as usize],
					"The root appears as a child"
				);
				ensure!(
					self.parents[node as usize]
						== ROOT_PARENT,
					"The root has a parent"
				);
			} else {
				ensure!(
					seen[node as usize],
					"Node {node} is not connected to a parent"
				);
			}
		}

		seen.fill(false);
		let mut stack = vec![root];
		while let Some(node) = stack.pop() {
			ensure!(
				!seen[node as usize],
				"The tree contains a cycle"
			);
			seen[node as usize] = true;
			if node >= num_leaves {
				let offset = (node - num_leaves) as usize * 2;
				stack.extend_from_slice(
					&self.children[offset..offset + 2],
				);
			}
		}
		ensure!(
			seen.iter().all(|value| *value),
			"Not all nodes are reachable from the root"
		);

		Ok(())
	}

	pub fn num_nodes(&self) -> u32 {
		self.num_leaves * 2 - 1
	}

	pub fn num_leaves(&self) -> u32 {
		self.num_leaves
	}

	pub fn num_internals(&self) -> u32 {
		self.num_leaves - 1
	}

	pub fn num_edges(&self) -> u32 {
		self.num_nodes() - 1
	}

	pub fn root(&self) -> Internal {
		Internal(self.root)
	}

	pub fn nodes(&self) -> impl DoubleEndedIterator<Item = Node> + use<> {
		(0..self.num_nodes()).map(Node)
	}

	pub fn leaves(&self) -> impl DoubleEndedIterator<Item = Leaf> + use<> {
		(0..self.num_leaves()).map(Leaf)
	}

	pub fn internals(
		&self,
	) -> impl DoubleEndedIterator<Item = Internal> + use<> {
		(self.num_leaves()..self.num_nodes()).map(Internal)
	}

	pub fn edges(&self) -> impl DoubleEndedIterator<Item = Node> + use<> {
		let root = self.root;
		(0..root).chain(root + 1..self.num_nodes()).map(Node)
	}

	pub fn is_leaf(&self, node: Node) -> bool {
		node.0 < self.num_leaves()
	}

	pub fn is_internal(&self, node: Node) -> bool {
		(self.num_leaves()..self.num_nodes()).contains(&node.0)
	}

	pub fn as_leaf(&self, node: Node) -> Option<Leaf> {
		self.is_leaf(node).then_some(Leaf(node.0))
	}

	pub fn as_internal(&self, node: Node) -> Option<Internal> {
		self.is_internal(node).then_some(Internal(node.0))
	}

	pub fn children_of(&self, node: Internal) -> [Node; 2] {
		let offset = (node.0 - self.num_leaves()) as usize * 2;
		[Node(self.children[offset]), Node(self.children[offset + 1])]
	}

	pub fn parent_of(&self, node: Node) -> Option<Internal> {
		let parent = self.parents[node.usize()];
		(parent != ROOT_PARENT).then_some(Internal(parent))
	}

	pub fn edge_length(&self, child: Node) -> Option<f64> {
		self.edge_index(child).map(|index| self.edge_lengths[index])
	}

	pub fn name(&self, node: Node) -> Option<&str> {
		self.node_names.get(node.usize())
	}

	pub fn node_metadata(&self, node: Node) -> Option<&str> {
		self.node_metadata.get(node.usize())
	}

	pub fn edge_metadata(&self, child: Node) -> Option<&str> {
		self.edge_index(child)
			.and_then(|index| self.edge_metadata.get(index))
	}

	fn edge_index(&self, child: Node) -> Option<usize> {
		(child.0 < self.num_nodes() && child.0 != self.root).then(
			|| (child.0 - u32::from(child.0 > self.root)) as usize,
		)
	}

	pub fn leaf_by_name(&self, name: &str) -> Option<Leaf> {
		self.leaves().find(|leaf| {
			self.node_names.get(leaf.0 as usize) == Some(name)
		})
	}

	pub fn mrca(&self, first: Node, second: Node) -> Node {
		let mut left = first.0;
		let mut right = second.0;

		while left != right {
			left = if left == ROOT_PARENT {
				second.0
			} else {
				self.parents[left as usize]
			};
			right = if right == ROOT_PARENT {
				first.0
			} else {
				self.parents[right as usize]
			};
		}

		Node(left)
	}

	pub fn ola(&self) -> Vec<i32> {
		let num_nodes = self.num_nodes();
		let num_leaves = self.num_leaves();

		let mut labels = Vec::from_iter(0..num_leaves as i32);
		labels.resize(num_nodes as usize, 0);

		let mut clade_founder = vec![0; num_nodes as usize];

		for node in self.postorder() {
			if let Some(leaf) = self.as_leaf(node) {
				clade_founder[leaf.usize()] = leaf.0
			} else if let Some(internal) = self.as_internal(node) {
				let [left, right] = self.children_of(internal);
				clade_founder[internal.usize()] = min(
					clade_founder[left.usize()],
					clade_founder[right.usize()],
				);
			} else {
				unreachable!()
			}
		}

		let mut clade_splitter = vec![0; num_nodes as usize];
		let mut splitter_to_node = vec![0; num_leaves as usize];
		for node in self.preorder() {
			let Some(internal) = self.as_internal(node) else {
				continue;
			};

			let [left, right] = self.children_of(internal);

			let splitter = max(
				clade_founder[left.usize()],
				clade_founder[right.usize()],
			);
			clade_splitter[node.usize()] = splitter;
			labels[node.usize()] = -(splitter as i32);
			splitter_to_node[splitter as usize] = node.0;
		}

		let mut ola = Vec::new();
		let mut forward_to = Vec::from_iter(0..num_nodes);

		for label in (1..num_leaves).rev() {
			let splitter_node =
				Internal(splitter_to_node[label as usize]);
			let [left, right] = self.children_of(splitter_node);

			let sibling = if clade_founder[left.usize()] == label {
				right
			} else {
				left
			};

			let mut curr = sibling;
			while forward_to[curr.usize()] != curr.0 {
				curr = Node(forward_to[curr.usize()]);
			}

			ola.push(labels[curr.usize()]);
			forward_to[splitter_node.usize()] = curr.0;
		}
		ola.reverse();

		ola
	}

	pub fn robinson_foulds(&self, other: &Self) -> u32 {
		super::distance::robinson_foulds(self, other)
	}

	pub fn triplet_distance(&self, other: &Self) -> u128 {
		assert_eq!(self.num_leaves(), other.num_leaves());

		if self.num_leaves() < 3 {
			return 0;
		}

		let subtree_sizes = self.triplet_subtree_sizes();
		let mut hdt = TripletHdt::new(other);
		self.triplet_distance_with_hdt(&subtree_sizes, &mut hdt)
	}

	fn triplet_subtree_sizes(&self) -> Vec<u32> {
		let mut subtree_sizes = vec![0; self.num_nodes() as usize];
		for node in self.postorder() {
			if self.is_leaf(node) {
				subtree_sizes[node.usize()] = 1;
			} else {
				let [left, right] = self.children_of(
					self.as_internal(node).unwrap(),
				);
				subtree_sizes[node.usize()] = subtree_sizes
					[left.usize()]
					+ subtree_sizes[right.usize()];
			}
		}
		subtree_sizes
	}

	fn triplet_distance_with_hdt(
		&self,
		subtree_sizes: &[u32],
		hdt: &mut TripletHdt,
	) -> u128 {
		let mut steps = vec![TripletStep::Count(self.root().into())];
		let mut leaves = Vec::new();
		let mut shared = 0;

		while let Some(step) = steps.pop() {
			match step {
				TripletStep::Count(node) => {
					let Some(internal) =
						self.as_internal(node)
					else {
						hdt.set_color(
							node.0,
							TripletColor::None,
						);
						continue;
					};

					let [left, right] =
						self.children_of(internal);
					let [small, large] = if subtree_sizes
						[left.usize()]
						<= subtree_sizes[right.usize()]
					{
						[left, right]
					} else {
						[right, left]
					};

					steps.push(TripletStep::Count(small));
					steps.push(TripletStep::Color(
						small,
						TripletColor::Red,
					));
					steps.push(TripletStep::Count(large));
					steps.push(TripletStep::Color(
						small,
						TripletColor::None,
					));
					steps.push(TripletStep::AddShared);
					steps.push(TripletStep::Color(
						small,
						TripletColor::Blue,
					));
				}
				TripletStep::Color(root, color) => {
					leaves.clear();
					leaves.push(root);
					while let Some(node) = leaves.pop() {
						if self.is_leaf(node) {
							hdt.set_color(
								node.0, color,
							);
						} else {
							let [left, right] = self.children_of(
								self.as_internal(node).unwrap(),
							);
							leaves.extend([
								left, right,
							]);
						}
					}
				}
				TripletStep::AddShared => {
					shared += hdt.shared()
				}
			}
		}

		choose3(self.num_leaves()) - shared
	}

	pub fn preorder(&self) -> impl Iterator<Item = Node> + '_ {
		Preorder {
			tree: self,
			stack: vec![self.root().into()],
		}
	}

	pub fn postorder(&self) -> impl Iterator<Item = Node> + '_ {
		Postorder {
			tree: self,
			stack: vec![(self.root().into(), false)],
		}
	}

	/// Returns `true` if the children have the same names and ids
	pub fn identical_children(&self, other: &BinaryTree) -> bool {
		self.node_names == other.node_names
	}
}

pub fn triplet_distance_matrix(
	trees: &[&BinaryTree],
) -> Result<Vec<Vec<u128>>> {
	let Some(first_tree) = trees.first() else {
		return Ok(Vec::new());
	};

	let num_leaves = first_tree.num_leaves();
	for tree in &trees[1..] {
		ensure!(
			tree.num_leaves() == num_leaves,
			"Expected every tree to have {num_leaves} leaves, got {}",
			tree.num_leaves()
		);
		ensure!(
			first_tree.identical_children(tree),
			"Expected every tree to use the same leaf IDs"
		);
	}

	let subtree_sizes = trees
		.iter()
		.map(|tree| tree.triplet_subtree_sizes())
		.collect::<Vec<_>>();
	let templates = trees
		.iter()
		.map(|tree| TripletHdt::new(tree))
		.collect::<Vec<_>>();
	let mut distances = vec![vec![0; trees.len()]; trees.len()];
	for first in 0..trees.len() {
		for second in 0..first {
			let mut hdt = templates[second].clone();
			let distance = trees[first].triplet_distance_with_hdt(
				&subtree_sizes[first],
				&mut hdt,
			);
			distances[first][second] = distance;
			distances[second][first] = distance;
		}
	}
	Ok(distances)
}

#[derive(Clone, Copy)]
enum TripletColor {
	None,
	Red,
	Blue,
}

enum TripletStep {
	Count(Node),
	Color(Node, TripletColor),
	AddShared,
}

#[derive(Clone, Copy, Default)]
struct TripletCounts {
	red: u32,
	blue: u32,
	same_red: u64,
	same_blue: u64,
	red_below_blue: u64,
	blue_below_red: u64,
	shared: u128,
}

impl TripletCounts {
	fn leaf(color: TripletColor) -> Self {
		match color {
			TripletColor::None => Self::default(),
			TripletColor::Red => Self {
				red: 1,
				..Self::default()
			},
			TripletColor::Blue => Self {
				blue: 1,
				..Self::default()
			},
		}
	}

	fn merge(lower: Self, upper: Self) -> Self {
		Self {
			red: lower.red + upper.red,
			blue: lower.blue + upper.blue,
			same_red: lower.same_red + upper.same_red,
			same_blue: lower.same_blue + upper.same_blue,
			red_below_blue: upper.red_below_blue
				+ lower.red_below_blue + u64::from(
				lower.red,
			) * u64::from(
				upper.blue,
			),
			blue_below_red: upper.blue_below_red
				+ lower.blue_below_red + u64::from(
				lower.blue,
			) * u64::from(
				upper.red,
			),
			shared: lower.shared
				+ upper.shared + u128::from(choose2(lower.red))
				* u128::from(upper.blue) + u128::from(choose2(
				lower.blue,
			)) * u128::from(
				upper.red,
			) + u128::from(upper.same_red)
				* u128::from(lower.blue) + u128::from(
				upper.same_blue,
			) * u128::from(
				lower.red,
			) + u128::from(lower.red)
				* u128::from(upper.red_below_blue)
				+ u128::from(lower.blue)
					* u128::from(upper.blue_below_red),
		}
	}

	fn attach_internal(lower: Self) -> Self {
		Self {
			red: lower.red,
			blue: lower.blue,
			same_red: choose2(lower.red),
			same_blue: choose2(lower.blue),
			shared: lower.shared,
			..Self::default()
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TripletComponentKind {
	Leaf,
	Internal,
	Path {
		lower: usize,
		upper: usize,
		upper_is_internal: bool,
	},
}

#[derive(Clone)]
struct TripletComponent {
	kind: TripletComponentKind,
	parent: usize,
	down_closed: bool,
	counts: TripletCounts,
}

impl TripletComponent {
	fn leaf(index: usize) -> Self {
		Self {
			kind: TripletComponentKind::Leaf,
			parent: index,
			down_closed: true,
			counts: TripletCounts::leaf(TripletColor::Red),
		}
	}

	fn internal(index: usize) -> Self {
		Self {
			kind: TripletComponentKind::Internal,
			parent: index,
			down_closed: false,
			counts: TripletCounts::default(),
		}
	}
}

#[derive(Clone, Copy)]
struct TripletEdge {
	up: usize,
	down: usize,
}

#[derive(Clone)]
struct TripletHdt {
	components: Vec<TripletComponent>,
	root: usize,
}

impl TripletHdt {
	fn new(tree: &BinaryTree) -> Self {
		let mut components =
			Vec::with_capacity(tree.num_nodes() as usize * 2 - 1);
		for node in tree.nodes() {
			let index = node.usize();
			components.push(if tree.is_leaf(node) {
				TripletComponent::leaf(index)
			} else {
				TripletComponent::internal(index)
			});
		}

		let mut edges = tree
			.edges()
			.map(|child| TripletEdge {
				up: tree.parent_of(child).unwrap().usize(),
				down: child.usize(),
			})
			.collect::<Vec<_>>();
		let mut next = Vec::with_capacity(edges.len());
		let mut root = tree.root().usize();

		while !edges.is_empty() {
			for edge in edges.drain(..) {
				if components[edge.up].parent != edge.up
					|| components[edge.down].parent
						!= edge.down
				{
					next.push(edge);
					continue;
				}

				let up_kind = components[edge.up].kind;
				let down_kind = components[edge.down].kind;
				let upper_is_internal = up_kind
					== TripletComponentKind::Internal;
				let path_merge = matches!(
					up_kind,
					TripletComponentKind::Path { .. }
				) && matches!(
					down_kind,
					TripletComponentKind::Path { .. }
						| TripletComponentKind::Leaf
				);
				let internal_merge = upper_is_internal
					&& components[edge.down].down_closed;

				if !path_merge && !internal_merge {
					next.push(edge);
					continue;
				}

				let index = components.len();
				let counts = if upper_is_internal {
					TripletCounts::attach_internal(
						components[edge.down].counts,
					)
				} else {
					TripletCounts::merge(
						components[edge.down].counts,
						components[edge.up].counts,
					)
				};
				let down_closed = if upper_is_internal {
					false
				} else {
					components[edge.down].down_closed
				};

				components[edge.up].parent = index;
				components[edge.down].parent = index;
				components.push(TripletComponent {
					kind: TripletComponentKind::Path {
						lower: edge.down,
						upper: edge.up,
						upper_is_internal,
					},
					parent: index,
					down_closed,
					counts,
				});
				root = index;
			}

			for edge in &mut next {
				edge.up = components[edge.up].parent;
				edge.down = components[edge.down].parent;
			}
			std::mem::swap(&mut edges, &mut next);
		}

		Self { components, root }
	}

	fn set_color(&mut self, leaf: u32, color: TripletColor) {
		let mut index = leaf as usize;
		self.components[index].counts = TripletCounts::leaf(color);

		while index != self.root {
			index = self.components[index].parent;
			let TripletComponentKind::Path {
				lower,
				upper,
				upper_is_internal,
			} = self.components[index].kind
			else {
				unreachable!()
			};
			self.components[index].counts = if upper_is_internal {
				TripletCounts::attach_internal(
					self.components[lower].counts,
				)
			} else {
				TripletCounts::merge(
					self.components[lower].counts,
					self.components[upper].counts,
				)
			};
		}
	}

	fn shared(&self) -> u128 {
		self.components[self.root].counts.shared
	}
}

fn choose2(value: u32) -> u64 {
	let value = u64::from(value);
	value * value.saturating_sub(1) / 2
}

fn choose3(value: u32) -> u128 {
	let value = u128::from(value);
	value * value.saturating_sub(1) * value.saturating_sub(2) / 6
}

struct Preorder<'a> {
	tree: &'a BinaryTree,
	stack: Vec<Node>,
}

impl Iterator for Preorder<'_> {
	type Item = Node;

	fn next(&mut self) -> Option<Self::Item> {
		let node = self.stack.pop()?;

		if let Some(internal) = self.tree.as_internal(node) {
			let [left, right] = self.tree.children_of(internal);
			self.stack.push(right);
			self.stack.push(left);
		}

		Some(node)
	}
}

struct Postorder<'a> {
	tree: &'a BinaryTree,
	stack: Vec<(Node, bool)>,
}

impl Iterator for Postorder<'_> {
	type Item = Node;

	fn next(&mut self) -> Option<Self::Item> {
		while let Some((node, children_visited)) = self.stack.pop() {
			if children_visited {
				return Some(node);
			}

			self.stack.push((node, true));
			if let Some(internal) = self.tree.as_internal(node) {
				let [left, right] =
					self.tree.children_of(internal);
				self.stack.push((right, false));
				self.stack.push((left, false));
			}
		}

		None
	}
}

#[cfg(test)]
mod tests {
	use super::BinaryTree;

	#[test]
	fn leaf_count_bounds() {
		assert!(BinaryTree::validate_num_leaves(0).is_err());
		assert!(BinaryTree::validate_num_leaves(1).is_err());
		assert!(BinaryTree::validate_num_leaves(2).is_ok());
		assert!(BinaryTree::validate_num_leaves(u32::MAX / 2).is_ok());
		assert!(BinaryTree::validate_num_leaves(u32::MAX / 2 + 1)
			.is_err());
		assert!(BinaryTree::validate_num_leaves(u32::MAX).is_err());
	}
}
