use anyhow::Result;
use parking_lot::{Mutex, MutexGuard};
use pyo3::{prelude::*, types::PyType};

use crate::tree::builder::TreeBuilder;

#[derive(Debug)]
#[pyclass(name = "Tree", module = "aspartik.data.tree", frozen)]
#[repr(transparent)]
pub struct PyTree {
	inner: Mutex<TreeBuilder>,
}

impl PyTree {
	pub fn inner(&self) -> MutexGuard<'_, TreeBuilder> {
		self.inner.lock()
	}
}

#[pymethods]
impl PyTree {
	#[new]
	fn new() -> Self {
		Self {
			inner: Mutex::new(TreeBuilder::new()),
		}
	}

	#[classmethod]
	fn from_newick(
		_class: &Bound<'_, PyType>,
		newick: &str,
	) -> Result<Self> {
		Ok(Self {
			inner: Mutex::new(TreeBuilder::parse_newick(newick)?),
		})
	}

	fn __str__(&self) -> Result<String> {
		self.inner().to_newick()
	}
}
