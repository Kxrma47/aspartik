use anyhow::Result;
use pyo3::{prelude::*, types::PyType};

use std::{borrow::BorrowMut, env};

use crate::{Transitions, parameters::DatedTree};

mod cpu4;
mod cuda_partials;
mod cuda_propagations;
mod state;

pub use cpu4::Cpu4Calculator;
pub use cuda_partials::CudaPartialsCalculator;
pub use cuda_propagations::CudaPropagationsCalculator;
pub use state::StateCalculator;

pub(super) fn cuda_path() -> String {
	env::var("CUDA_PATH")
		.or_else(|_| env::var("CUDA_HOME"))
		.unwrap_or_else(|_| "/usr/local/cuda".to_string())
}

pub(super) fn cccl_qual_path() -> String {
	let arch = match env::consts::ARCH {
		"x86_64" => "x86_64",
		_ => unimplemented!(),
	};
	let os = env::consts::OS;
	format!("{}/targets/{arch}-{os}/include/cccl/", cuda_path())
}

/// Felsenstein's pruning tree likelihood calculator
///
/// This trait is for low-level implementations of Felsenstein's likelihood
/// algorithm which are used by `GenericLikelihood`.  The latter takes care of
/// the substitution model and the tree, so a calculator only needs to implement
/// the raw calculations.
///
/// The trait is generic over `N`, the dimensionality of data, and `F`, the
/// floating point type.  For now all likelihoods use `N = 4` (DNA nucleotides)
/// and `f64`.
///
/// Calculators have a simple life cycle.  On a step where either the tree, the
/// clock rate, or the substitution models are edited, `GenericLikelihood`
/// figures out the minimal set of nodes to update, the transition matrices
/// relevant for the update, and calls `propose`.  After that it calls
/// `likelihood`, and, finally, either `accept` or `reject`.
///
/// `propose` and `likelihood` are split into two different methods to allow
/// asynchronous implementations.  Some higher-level likelihoods compose over
/// several calculators.  In this case, they'll first call `propose` on each
/// calculator and then block on `likelihood` calls.
pub trait Calculator<F> {
	/// Calculate tree likelihood
	fn propose(
		&mut self,
		tree: &DatedTree,
		transitions: &Transitions,
	) -> Result<()>;

	/// Accept the changes made in `likelihood`
	fn accept(&mut self) -> Result<()>;

	/// Reject the changes made in `likelihood`
	///
	/// This should roll back the internal state of the calculator to
	/// exactly what it was after the last call to `accept`.
	fn reject(&mut self) -> Result<()>;

	/// Pattern likelihoods calculated after the last update
	fn likelihoods(&self) -> &[f64];

	/// Number of patterns in the alignment
	fn num_patterns(&self) -> usize;
}

#[derive(Debug, Clone, Copy)]
enum CalculatorKind {
	Cpu {
		num_threads: usize,
	},
	CUDAPropagations {
		device: usize,
		prop_block_size: u32,
		leaf_block_size: u32,
	},
	CUDAPartials {
		device: usize,
		block_size: u32,
	},
}

#[derive(Debug, Clone, Copy)]
#[pyclass(name = "Calculator", module = "aspartik.b3", from_py_object)]
pub struct CalculatorConfig {
	kind: CalculatorKind,
	scale_ln: u32,
}

#[pymethods]
impl CalculatorConfig {
	#[pyo3(name = "CPU", signature = (num_threads = 0))]
	#[classmethod]
	fn cpu(_cls: Py<PyType>, num_threads: usize) -> Self {
		Self {
			kind: CalculatorKind::Cpu { num_threads },
			scale_ln: 30,
		}
	}

	#[pyo3(
		name = "CUDAPropagations",
		signature = (device = 0, prop_block_size = 64, leaf_block_size = 256)
	)]
	#[classmethod]
	fn cuda_propagations(
		_cls: Py<PyType>,
		device: usize,
		prop_block_size: u32,
		leaf_block_size: u32,
	) -> Self {
		Self {
			kind: CalculatorKind::CUDAPropagations {
				device,
				prop_block_size,
				leaf_block_size,
			},
			scale_ln: 30,
		}
	}

	#[pyo3(name = "CUDAPartials", signature = (device = 0, block_size = 32))]
	#[classmethod]
	fn cuda_partials(
		_cls: Py<PyType>,
		device: usize,
		block_size: u32,
	) -> Self {
		Self {
			kind: CalculatorKind::CUDAPartials {
				device,
				block_size,
			},
			scale_ln: 30,
		}
	}

	fn with_scale(
		mut this: PyRefMut<'_, Self>,
		scale_ln: u32,
	) -> PyRefMut<'_, Self> {
		this.borrow_mut().scale_ln = scale_ln;
		this
	}
}

impl CalculatorConfig {
	pub fn make4(
		&self,
		samples: Vec<u8>,
		num_patterns: usize,
	) -> Result<Box<dyn Calculator<f64> + Send + Sync>> {
		match self.kind {
			CalculatorKind::Cpu { num_threads } => {
				let calc = Cpu4Calculator::new(
					num_patterns,
					samples,
					self.scale_ln,
					num_threads,
				);
				Ok(Box::new(calc))
			}
			CalculatorKind::CUDAPropagations {
				device,
				prop_block_size,
				leaf_block_size,
			} => {
				let calc = CudaPropagationsCalculator::new(
					num_patterns,
					samples,
					self.scale_ln,
					prop_block_size,
					leaf_block_size,
					device,
				)?;
				Ok(Box::new(calc))
			}
			CalculatorKind::CUDAPartials { device, block_size } => {
				let calc = CudaPartialsCalculator::new(
					num_patterns,
					samples,
					self.scale_ln,
					block_size,
					device,
				)?;
				Ok(Box::new(calc))
			}
		}
	}
}
