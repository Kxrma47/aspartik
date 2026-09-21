use anyhow::{Result, ensure};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::SmallVec;

use super::{BinaryTree, Node};

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
struct CladeHash {
	first: u64,
	second: u64,
	size: u32,
}

impl CladeHash {
	fn leaf(index: u32) -> Self {
		let index = u64::from(index) + 1;
		Self {
			first: mix(index ^ 0x243f_6a88_85a3_08d3),
			second: mix(index ^ 0x1319_8a2e_0370_7344),
			size: 1,
		}
	}

	fn combine(self, other: Self) -> Self {
		Self {
			first: self.first.wrapping_add(other.first),
			second: self.second.wrapping_add(other.second),
			size: self.size + other.size,
		}
	}
}

fn mix(mut value: u64) -> u64 {
	value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
	value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
	value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
	value ^ (value >> 31)
}

fn clade_hashes(tree: &BinaryTree) -> Vec<CladeHash> {
	let mut hashes = vec![CladeHash::default(); tree.num_nodes() as usize];
	for node in tree.postorder() {
		let hash = clade_hash(tree, &hashes, node);
		hashes[node.usize()] = hash;
	}
	hashes
}

pub(super) fn robinson_foulds(first: &BinaryTree, second: &BinaryTree) -> u32 {
	assert_eq!(first.num_leaves(), second.num_leaves());
	let first_hashes = clade_hashes(first);
	let second_hashes = clade_hashes(second);
	let clades = first
		.internals()
		.filter(|&node| node != first.root())
		.map(|node| first_hashes[node.usize()])
		.collect::<FxHashSet<_>>();
	let shared = second
		.internals()
		.filter(|&node| node != second.root())
		.filter(|&node| clades.contains(&second_hashes[node.usize()]))
		.count() as u32;
	2 * (first.num_leaves() - 2 - shared)
}

fn clade_hash(
	tree: &BinaryTree,
	hashes: &[CladeHash],
	node: Node,
) -> CladeHash {
	if let Some(leaf) = tree.as_leaf(node) {
		CladeHash::leaf(leaf.u32())
	} else {
		let internal = tree.as_internal(node).unwrap();
		let [left, right] = tree.children_of(internal);
		hashes[left.usize()].combine(hashes[right.usize()])
	}
}

/// Computes all rooted Robinson-Foulds distances by indexing shared clades.
///
/// Expected time is O(kn + sum(f_c^2)) for k trees with n leaves, where f_c
/// is the number of trees containing clade c. See
/// <https://doi.org/10.1016/0025-5564(81)90043-2>.
pub fn robinson_foulds_matrix(trees: &[&BinaryTree]) -> Result<Vec<Vec<u32>>> {
	let Some(first_tree) = trees.first() else {
		return Ok(Vec::new());
	};

	let num_leaves = first_tree.num_leaves();
	for tree in trees[1..].iter() {
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

	let num_clades = usize::try_from(num_leaves - 2)?;
	let capacity = trees.len().checked_mul(num_clades).unwrap_or(0);
	let mut clades =
		FxHashMap::<CladeHash, SmallVec<[usize; 2]>>::with_capacity_and_hasher(
			capacity,
			FxBuildHasher,
		);
	let mut hashes =
		vec![CladeHash::default(); first_tree.num_nodes() as usize];

	for (tree_index, tree) in trees.iter().enumerate() {
		for node in tree.postorder() {
			let hash = clade_hash(tree, &hashes, node);
			hashes[node.usize()] = hash;
			if node != tree.root().into() && tree.is_internal(node)
			{
				let tree_indices =
					clades.entry(hash).or_default();
				if tree_indices.last() != Some(&tree_index) {
					tree_indices.push(tree_index);
				}
			}
		}
	}

	let max_distance = (num_leaves - 2) * 2;
	let mut distances = vec![vec![max_distance; trees.len()]; trees.len()];
	for (index, row) in distances.iter_mut().enumerate() {
		row[index] = 0;
	}

	for tree_indices in clades.values() {
		for (offset, &first) in tree_indices.iter().enumerate() {
			for &second in &tree_indices[offset + 1..] {
				distances[first][second] -= 2;
				distances[second][first] -= 2;
			}
		}
	}

	Ok(distances)
}

/// Computes the Kuhner-Felsenstein branch-score distance from hashed clades.
///
/// Expected time is O(n) for trees with n leaves. See
/// <https://doi.org/10.1093/oxfordjournals.molbev.a040126>.
pub fn branch_score(first: &BinaryTree, second: &BinaryTree) -> Result<f64> {
	ensure!(
		first.num_leaves() == second.num_leaves(),
		"Expected both trees to have {} leaves, got {}",
		first.num_leaves(),
		second.num_leaves()
	);
	let first_hashes = clade_hashes(first);
	let second_hashes = clade_hashes(second);
	let mut lengths = FxHashMap::with_capacity_and_hasher(
		first.num_edges() as usize,
		FxBuildHasher,
	);
	for child in first.edges() {
		lengths.insert(
			first_hashes[child.usize()],
			first.edge_length(child).unwrap(),
		);
	}

	let mut squared = 0.0;
	for child in second.edges() {
		let length = second.edge_length(child).unwrap();
		let first_length = lengths
			.remove(&second_hashes[child.usize()])
			.unwrap_or(0.0);
		squared += (first_length - length).powi(2);
	}
	squared += lengths.values().map(|length| length.powi(2)).sum::<f64>();
	Ok(squared.sqrt())
}

impl BinaryTree {
	/// Computes the rooted Robinson-Foulds distance from hashed clades.
	///
	/// Expected time is O(n) for trees with n leaves. See
	/// <https://doi.org/10.1016/0025-5564(81)90043-2>.
	pub fn robinson_foulds(&self, other: &Self) -> u32 {
		robinson_foulds(self, other)
	}

	/// Computes triplet distance using smaller-half recoloring and an HDT.
	///
	/// Time is O(n log^2 n) for trees with n leaves. See
	/// <https://doi.org/10.1186/1471-2105-14-S2-S18>.
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
}

/// Computes all pairwise triplet distances using HDT templates.
///
/// Time is O(k^2 n log^2 n) for k trees with n leaves. See
/// <https://doi.org/10.1186/1471-2105-14-S2-S18>.
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
