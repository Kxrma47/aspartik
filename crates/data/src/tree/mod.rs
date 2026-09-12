mod binary;
pub mod builder;
mod distance;
mod newick;
mod parse_newick;
#[cfg(feature = "python")]
pub mod python;
mod serialize_newick;

pub use binary::BinaryTree;
pub use distance::{branch_score, branch_score_matrix, robinson_foulds_matrix};
pub use parse_newick::parse as parse_newick;

const ROOT_PARENT: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Node(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Leaf(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Internal(u32);

impl Node {
	pub fn index(self) -> u32 {
		self.0
	}

	fn i(self) -> usize {
		self.0 as usize
	}
}

impl Leaf {
	pub fn index(self) -> u32 {
		self.0
	}

	fn i(self) -> usize {
		self.0 as usize
	}
}

impl Internal {
	pub fn index(self) -> u32 {
		self.0
	}

	fn i(self) -> usize {
		self.0 as usize
	}
}

impl From<Leaf> for Node {
	fn from(value: Leaf) -> Self {
		Self(value.0)
	}
}

impl From<Internal> for Node {
	fn from(value: Internal) -> Self {
		Self(value.0)
	}
}
