use anyhow::{Result, ensure};
use rustc_hash::{FxBuildHasher, FxHashMap};

use super::BinaryTree;

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
		hashes[node.index() as usize] =
			if let Some(leaf) = tree.as_leaf(node) {
				CladeHash::leaf(leaf.index())
			} else {
				let internal = tree.as_internal(node).unwrap();
				let (left, right) = tree.children_of(internal);
				hashes[left.index() as usize]
					.combine(hashes[right.index() as usize])
			};
	}
	hashes
}

pub fn robinson_foulds_matrix(trees: &[BinaryTree]) -> Result<Vec<Vec<u32>>> {
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
	}

	let num_clades = usize::try_from(num_leaves - 2)?;
	let capacity = trees.len().checked_mul(num_clades).unwrap_or(0);
	let mut clades =
		FxHashMap::<CladeHash, Vec<usize>>::with_capacity_and_hasher(
			capacity,
			FxBuildHasher,
		);

	for (tree_index, tree) in trees.iter().enumerate() {
		let hashes = clade_hashes(tree);
		for node in tree.postorder() {
			if node != tree.root().into() && tree.is_internal(node)
			{
				let tree_indices = clades
					.entry(hashes[node.index() as usize])
					.or_default();
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
			first_hashes[child.index() as usize],
			first.edge_length(child).unwrap(),
		);
	}
	let mut squared = 0.0;
	for child in second.edges() {
		let length = second.edge_length(child).unwrap();
		let first_length = lengths
			.remove(&second_hashes[child.index() as usize])
			.unwrap_or(0.0);
		squared += (first_length - length).powi(2);
	}
	squared += lengths.values().map(|length| length.powi(2)).sum::<f64>();
	Ok(squared.sqrt())
}

pub fn branch_score_matrix(trees: &[BinaryTree]) -> Result<Vec<Vec<f64>>> {
	let Some(first) = trees.first() else {
		return Ok(Vec::new());
	};
	for tree in &trees[1..] {
		ensure!(
			tree.num_leaves() == first.num_leaves(),
			"Expected every tree to have {} leaves, got {}",
			first.num_leaves(),
			tree.num_leaves()
		);
	}

	let mut clade_indices = FxHashMap::with_capacity_and_hasher(
		trees.len() * first.num_edges() as usize,
		FxBuildHasher,
	);
	let mut tree_lengths = Vec::with_capacity(trees.len());
	for tree in trees {
		let hashes = clade_hashes(tree);
		let mut lengths = Vec::with_capacity(tree.num_edges() as usize);
		for child in tree.edges() {
			let next_index = clade_indices.len();
			let index = *clade_indices
				.entry(hashes[child.index() as usize])
				.or_insert(next_index);
			lengths.push((index, tree.edge_length(child).unwrap()));
		}
		tree_lengths.push(lengths);
	}

	let mut values = vec![vec![0.0; clade_indices.len()]; trees.len()];
	for (row, lengths) in values.iter_mut().zip(tree_lengths) {
		for (index, length) in lengths {
			row[index] = length;
		}
	}
	let mut distances = vec![vec![0.0; trees.len()]; trees.len()];
	for first in 0..trees.len() {
		for second in first + 1..trees.len() {
			let squared = values[first]
				.iter()
				.zip(&values[second])
				.map(|(left, right)| (left - right).powi(2))
				.sum::<f64>();
			let distance = squared.sqrt();
			distances[first][second] = distance;
			distances[second][first] = distance;
		}
	}
	Ok(distances)
}
