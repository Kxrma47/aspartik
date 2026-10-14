#![allow(clippy::clone_on_copy)]

mod binary;
pub mod builder;
mod distances;
mod newick;
#[cfg(feature = "python")]
pub mod python;
mod render;

pub use binary::BinaryTree;
pub use builder::TreeBuilder;
pub use distances::{
	branch_score, branch_score_matrix, robinson_foulds_matrix,
	triplet_distance_matrix,
};
pub use render::{LayoutKind, Point, SvgOptions, TreeLayout};

use std::{fmt, mem, ops::Deref};

const TOMBSTONE: u32 = 0x544f4f52;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
	feature = "python",
	pyo3::pyclass(
		module = "aspartik.data.tree",
		frozen,
		skip_from_py_object
	)
)]
#[repr(transparent)]
pub struct Node(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
	feature = "python",
	pyo3::pyclass(module = "aspartik.data.tree", frozen, from_py_object)
)]
#[repr(transparent)]
pub struct Leaf(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
	feature = "python",
	pyo3::pyclass(module = "aspartik.data.tree", frozen, from_py_object)
)]
#[repr(transparent)]
pub struct Internal(u32);

impl Node {
	const TOMBSTONE: Self = Node(TOMBSTONE);
}

macro_rules! nodes_3 {
	($type:ty) => {
		impl $type {
			pub fn u32(self) -> u32 {
				self.0
			}

			pub fn usize(self) -> usize {
				self.0 as usize
			}
		}
	};
}

nodes_3!(Node);
nodes_3!(Leaf);
nodes_3!(Internal);

macro_rules! nodes_2 {
	($type:ty) => {
		impl Deref for $type {
			type Target = Node;

			fn deref<'a>(&'a self) -> &'a Node {
				// SAFETY: `Leaf`, `Internal`, and `Node` all
				// have the same layout with: `u32` with
				// `repr(transparent)`.  So, casting between
				// them while preserving the lifetimes is safe.
				unsafe {
					mem::transmute::<&'a $type, &'a Node>(
						self,
					)
				}
			}
		}

		impl From<$type> for Node {
			fn from(value: $type) -> Node {
				Node(value.0)
			}
		}
	};
}

nodes_2!(Leaf);
nodes_2!(Internal);

pub trait Tree {
	fn root(&self) -> Option<Node>;

	fn parent_of(&self, node: Node) -> Option<Node>;

	fn children_of(&self, node: Node) -> &[Node];

	fn name_of(&self, node: Node) -> &str;

	fn edge_length(&self, node: Node) -> Option<f64>;

	fn mrca(&self, mut first: Node, mut second: Node) -> Node {
		while first != second {
			first = self.parent_of(first).unwrap_or(second);
			second = self.parent_of(second).unwrap_or(first);
		}
		first
	}

	fn to_newick(&self) -> String {
		struct AsDisplay<'a, T: ?Sized>(&'a T);
		impl<T: Tree + ?Sized> fmt::Display for AsDisplay<'_, T> {
			fn fmt(
				&self,
				f: &mut fmt::Formatter<'_>,
			) -> fmt::Result {
				newick::serialize_newick(self.0, f)
			}
		}
		AsDisplay(self).to_string()
	}
}

// TODO: generic preorder/postorder, perhaps a RootedTree traits
