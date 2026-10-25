use crate::{Matrix as _, MatrixRef};

pub trait MatrixArrayExt {
	type T;

	fn as_mat(&self) -> &MatrixRef<Self::T>;
	fn as_mat_mut(&mut self) -> &mut MatrixRef<Self::T>;
}

impl<T, const N: usize, const M: usize> MatrixArrayExt for [[T; M]; N] {
	type T = T;

	fn as_mat(&self) -> &MatrixRef<T> {
		MatrixRef::from_slice(self.as_flattened(), N, M)
	}

	fn as_mat_mut(&mut self) -> &mut MatrixRef<T> {
		MatrixRef::from_slice_mut(self.as_flattened_mut(), N, M)
	}
}

pub trait MatrixSliceExt {
	type T;

	fn as_mat(&self, rows: usize, cols: usize) -> &MatrixRef<Self::T>;
	fn as_mat_mut(
		&mut self,
		rows: usize,
		cols: usize,
	) -> &mut MatrixRef<Self::T>;
}

impl<T> MatrixSliceExt for [T] {
	type T = T;

	fn as_mat(&self, rows: usize, cols: usize) -> &MatrixRef<T> {
		MatrixRef::from_slice(self, rows, cols)
	}

	fn as_mat_mut(
		&mut self,
		rows: usize,
		cols: usize,
	) -> &mut MatrixRef<T> {
		MatrixRef::from_slice_mut(self, rows, cols)
	}
}

pub trait MatrixCopyExt {
	fn copy_from(&mut self, src: &Self);
}

impl<T: Copy> MatrixCopyExt for MatrixRef<T> {
	fn copy_from(&mut self, src: &Self) {
		assert_eq!(self.num_rows(), src.num_rows());
		assert_eq!(self.num_cols(), src.num_cols());

		for i in 0..self.num_rows() {
			for j in 0..self.num_cols() {
				*self.at_mut(i, j) = *src.at(i, j);
			}
		}
	}
}
