use anyhow::{Context, Result, anyhow, bail, ensure};
use bytemuck::cast_slice;
use cudarc::{
	driver::{
		CudaContext, CudaFunction, CudaSlice, CudaStream, LaunchConfig,
		PushKernelArg, sys::is_culib_present,
	},
	nvrtc::{CompileOptions, compile_ptx_with_opts},
};

use std::sync::Arc;

use super::{
	Calculator,
	cuda::{cccl_qual_path, cuda_path},
};
use crate::{Transitions, parameters::DatedTree};
use sk::EditBuf;

type Row = [f64; 4];
type Transition = [f64; 16];

const CUDA_SRC: &str =
	concat!(include_str!("typedefs.h"), include_str!("kernels.cu"),);

/// CUDA calculator with the same update strategy as `Cpu4Calculator`
///
/// Instead of propagating partial likelihoods through the tree, it
/// recomputes partials at internal nodes using the full postorder list.
pub struct Cuda2Calculator {
	stream: Arc<CudaStream>,

	propose_fn: CudaFunction,

	selectors: EditBuf,
	selectors_device: CudaSlice<u8>,

	/// Leaf likelihoods, grouped by leaves
	leaves: CudaSlice<u8>,

	/// Partial likelihoods at internal nodes
	///
	/// It has the length of `num_internals * num_patterns * 2`.  Each
	/// internal node has two `num_patterns`-long buffers, determined by
	/// the selector.
	partials: CudaSlice<Row>,

	/// `num_patterns`-long root likelihoods
	likelihoods: CudaSlice<f64>,
	likelihoods_host: Box<[f64]>,

	/// Children of internal nodes updated in the current proposal
	children: CudaSlice<[u32; 2]>,

	transitions_host: Box<[f64]>,
	/// Transitions for each edge from `internals`
	transitions: CudaSlice<Transition>,

	/// Internal nodes updated in the current proposal
	internals: CudaSlice<u32>,

	scales: CudaSlice<u8>,
	scale_sums: CudaSlice<u32>,
	scale_sums_backup: CudaSlice<u32>,

	num_patterns: u32,

	scale_ln: u32,
}

impl Calculator<f64> for Cuda2Calculator {
	fn propose(
		&mut self,
		tree: &DatedTree,
		transitions: &Transitions,
	) -> Result<()> {
		let frequencies =
			*transitions.frequencies().as_array::<4>().unwrap();

		let (internals, children) = tree.partials_lists();
		transitions.write_matrices(
			cast_slice(&children),
			&mut self.transitions_host,
		);

		for &node in &internals {
			let idx = node - (self.selectors.len() as u32 + 1);
			self.selectors.set_edited(idx as usize);
		}
		self.stream.memcpy_htod(
			self.selectors.as_slice(),
			&mut self.selectors_device,
		)?;

		self.stream.memcpy_htod(&internals, &mut self.internals)?;
		self.stream.memcpy_htod(&children, &mut self.children)?;

		let tms = cast_slice(&self.transitions_host);
		self.stream.memcpy_htod(tms, &mut self.transitions)?;

		self.update_partials(internals.len() as u32, frequencies)?;

		// blocks on the running kernels
		self.stream.memcpy_dtoh(
			&self.likelihoods,
			&mut *self.likelihoods_host,
		)?;

		let scale_sums = self.stream.clone_dtoh(&self.scale_sums)?;
		for (likelihood, scale) in
			self.likelihoods_host.iter_mut().zip(scale_sums)
		{
			*likelihood -= f64::from(scale * self.scale_ln);
		}

		Ok(())
	}

	fn accept(&mut self) -> Result<()> {
		self.stream.memcpy_dtod(
			&self.scale_sums,
			&mut self.scale_sums_backup,
		)?;

		self.selectors.accept();

		Ok(())
	}

	fn reject(&mut self) -> Result<()> {
		self.stream.memcpy_dtod(
			&self.scale_sums_backup,
			&mut self.scale_sums,
		)?;

		self.selectors.reject();

		Ok(())
	}

	fn likelihoods(&self) -> &[f64] {
		&self.likelihoods_host
	}

	fn num_patterns(&self) -> usize {
		self.num_patterns as usize
	}
}

impl Cuda2Calculator {
	fn update_partials(
		&self,
		num_internals: u32,
		frequencies: Row,
	) -> Result<()> {
		let mut builder = self.stream.launch_builder(&self.propose_fn);

		let cfg = self.cfg(128);

		builder.arg(&self.leaves);
		builder.arg(&self.partials);
		builder.arg(&self.scales);
		builder.arg(&self.scale_sums);
		builder.arg(&self.selectors_device);

		builder.arg(&num_internals);
		builder.arg(&self.internals);
		builder.arg(&self.children);
		builder.arg(&self.transitions);
		builder.arg(&self.likelihoods);
		builder.arg(&frequencies);

		// SAFETY: TODO
		unsafe { builder.launch(cfg) }
			.with_context(|| anyhow!("update_partials: {cfg:?}"))?;

		Ok(())
	}

	fn cfg(&self, block_size: u32) -> LaunchConfig {
		let num_site_blocks = self.num_patterns.div_ceil(block_size);
		LaunchConfig {
			grid_dim: (num_site_blocks, 1, 1),
			block_dim: (block_size, 1, 1),
			shared_mem_bytes: 0,
		}
	}

	pub fn new(
		num_patterns: usize,
		mut leaves: Vec<u8>,
		scale_ln: u32,
		cuda_device: usize,
	) -> Result<Self> {
		// SAFETY: since the function tries to link the library [0], it
		// could theoretically be unsafe.  In practice, the resulting
		// library is discarded.
		//
		// [0]: https://docs.rs/cudarc/0.17.3/src/cudarc/driver/sys/mod.rs.html#26256
		let is_cuda_enabled = unsafe { is_culib_present() };
		if !is_cuda_enabled {
			bail!("CUDA library not found");
		}

		let scale_threshold = f64::from(-(scale_ln as i32)).exp();
		let scale_mult = f64::from(scale_ln).exp();

		for sample in &mut leaves {
			*sample = match *sample {
				0b0001 => 0,
				0b0010 => 1,
				0b0100 => 2,
				0b1000 => 3,
				_ => 4,
			};
		}

		let num_leaves = leaves.len() / num_patterns;
		let num_internals = num_leaves - 1;
		let num_edges = num_internals * 2;

		let context = CudaContext::new(cuda_device)?;
		let required =
			num_patterns * (num_leaves + num_internals * 2 * 4 * 2);
		let available = context.total_mem()?;
		ensure!(
			required < available,
			"The calculator requires at least {required}b of memory, but the device only has {available}b"
		);

		let stream = context.new_stream()?;

		// SAFETY: `Cuda2Calculator` only uses a single stream, so
		// there's no need for cross-stream synchronization
		unsafe { context.disable_event_tracking() };

		let leaves = stream.clone_htod(&leaves)?;

		let partials: CudaSlice<Row> =
			stream.alloc_zeros(num_internals * num_patterns * 2)?;

		let likelihoods: CudaSlice<f64> =
			stream.alloc_zeros(num_patterns)?;
		let children = stream.alloc_zeros(num_internals)?;
		let transitions = stream.alloc_zeros(num_edges)?;
		let internals = stream.alloc_zeros(num_internals)?;

		let selectors = EditBuf::new(num_internals);
		let selectors_device =
			stream.clone_htod(selectors.as_slice())?;

		let scales =
			stream.alloc_zeros(num_internals * num_patterns * 2)?;
		let scale_sums = stream.alloc_zeros(num_patterns)?;
		let scale_sums_backup = stream.alloc_zeros(num_patterns)?;

		let opts = CompileOptions {
			include_paths: vec![
				format!("{}/include", cuda_path()),
				format!("{}/include/cccl", cuda_path()),
				cccl_qual_path(),
			],
			options: vec![
				format!("-DNUM_PATTERNS={num_patterns}"),
				format!("-DNUM_LEAVES={num_leaves}"),
				format!("-DSCALE_LN={scale_ln}"),
				format!("-DSCALE_THRESHOLD={scale_threshold}"),
				format!("-DSCALE_MULT={scale_mult}"),
			],
			..Default::default()
		};
		let ptx = compile_ptx_with_opts(CUDA_SRC, opts)?;

		let module = context.load_module(ptx)?;
		let propose_fn = module.load_function("propose")?;
		Ok(Self {
			stream,

			propose_fn,

			selectors,
			selectors_device,

			leaves,
			partials,
			likelihoods,
			likelihoods_host: vec![0.0; num_patterns].into(),
			children,
			transitions_host: vec![0.0; num_edges * 16].into(),
			transitions,
			internals,

			scales,
			scale_sums,
			scale_sums_backup,

			scale_ln,

			num_patterns: num_patterns as u32,
		})
	}
}
