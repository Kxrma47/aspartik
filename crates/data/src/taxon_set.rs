use picoarrow::array::{Array, ArrayUtf8, NonNullable};
#[cfg(feature = "python")]
use pyo3::{prelude::*, types::PyType};

use std::{iter::FromIterator, ops::Deref, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaxonSet {
	inner: Arc<ArrayUtf8<NonNullable>>,
}

impl TaxonSet {
	pub fn new(names: ArrayUtf8<NonNullable>) -> Self {
		TaxonSet {
			inner: Arc::new(names),
		}
	}

	pub fn ranged_ints(len: usize) -> Self {
		Self::from_iter((0..len).map(|i| i.to_string()))
	}

	pub fn iter(&self) -> TaxonSetIter<'_> {
		TaxonSetIter {
			taxon_set: self,
			index: 0,
		}
	}
}

impl<S> FromIterator<S> for TaxonSet
where
	S: AsRef<str>,
{
	fn from_iter<I>(names: I) -> Self
	where
		I: IntoIterator<Item = S>,
	{
		let names = names.into_iter();
		let len = names.size_hint().1.unwrap_or(0);
		let mut array = ArrayUtf8::<NonNullable>::with_capacity(len);
		for name in names {
			array.push(name.as_ref()).unwrap();
		}
		Self::new(array)
	}
}

impl From<ArrayUtf8<NonNullable>> for TaxonSet {
	fn from(value: ArrayUtf8<NonNullable>) -> Self {
		Self::new(value)
	}
}

impl Deref for TaxonSet {
	type Target = ArrayUtf8<NonNullable>;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

// TODO: replace with upstream `picoarrow` iterator
pub struct TaxonSetIter<'a> {
	taxon_set: &'a TaxonSet,
	index: usize,
}

impl<'a> Iterator for TaxonSetIter<'a> {
	type Item = &'a str;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.taxon_set.len() {
			let item = self.taxon_set.get(self.index);
			self.index += 1;
			Some(item)
		} else {
			None
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.taxon_set.len() - self.index;
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for TaxonSetIter<'_> {}

impl<'a> IntoIterator for &'a TaxonSet {
	type Item = &'a str;
	type IntoIter = TaxonSetIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		TaxonSetIter {
			taxon_set: self,
			index: 0,
		}
	}
}

#[cfg(feature = "python")]
#[derive(Debug, Clone, PartialEq, Eq)]
#[pyclass(
	from_py_object,
	name = "TaxonSet",
	module = "aspartik.data",
	frozen,
	eq
)]
#[repr(transparent)]
pub struct PyTaxonSet(pub TaxonSet);

#[cfg(feature = "python")]
#[pymethods]
impl PyTaxonSet {
	#[new]
	fn new(names: Vec<String>) -> Self {
		PyTaxonSet(TaxonSet::from_iter(names))
	}

	#[classmethod]
	fn ranged_ints(_cls: Py<PyType>, len: usize) -> Self {
		PyTaxonSet(TaxonSet::ranged_ints(len))
	}

	// TODO: replace with a Python-native iterator
	fn to_list(&self) -> Vec<String> {
		self.0.iter().map(|s| s.to_owned()).collect()
	}
}
