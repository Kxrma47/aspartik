use super::{BinaryTree, Node};

impl BinaryTree {
	pub fn nhx(&self, node: Node, key: &str) -> Option<&str> {
		let mut metadata = self.node_metadata(node)?;
		while let Some(start) = metadata.find("[&&NHX") {
			let tagged = &metadata[start + 6..];
			let end = tagged.find(']')?;
			let fields = tagged[..end]
				.strip_prefix(':')
				.unwrap_or(&tagged[..end]);
			for field in fields.split(':') {
				let Some((field_key, value)) =
					field.split_once('=')
				else {
					continue;
				};
				if field_key == key {
					return Some(value);
				}
			}
			metadata = &tagged[end + 1..];
		}
		None
	}
}
