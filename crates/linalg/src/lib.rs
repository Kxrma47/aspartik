#[cfg(feature = "arbitrary")]
pub mod arbitrary;
pub mod beast_eigen;
mod dim;

pub use computare_linalg::{Matrix, MatrixRef};
pub use dim::Dim;

pub fn mul(
	lhs: &MatrixRef<f64>,
	rhs: &MatrixRef<f64>,
	dst: &mut MatrixRef<f64>,
) {
	assert_eq!(lhs.num_cols(), rhs.num_rows());
	assert_eq!(dst.num_rows(), lhs.num_rows());
	assert_eq!(dst.num_cols(), rhs.num_cols());

	let rows = dst.num_rows();
	let cols = dst.num_cols();
	let inner = lhs.num_cols();

	for i in 0..rows {
		for j in 0..cols {
			let mut sum = 0.0;
			for k in 0..inner {
				sum += lhs.at(i, k) * rhs.at(k, j);
			}
			*dst.at_mut(i, j) = sum;
		}
	}
}
