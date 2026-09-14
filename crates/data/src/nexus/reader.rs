use anyhow::{Context, Result, ensure};

use std::io::{BufRead, Cursor};

use super::{BlockReader, TranslationTable, TreeCommand};
use crate::tree::{builder::TreeBuilder, parse_newick};

#[derive(Debug, Clone)]
pub struct NexusTree {
	name: String,
	is_default: bool,
	is_rooted: Option<bool>,
	tree: TreeBuilder,
	line: usize,
	column: usize,
}

impl NexusTree {
	pub fn name(&self) -> &str {
		&self.name
	}

	pub fn is_default(&self) -> bool {
		self.is_default
	}

	pub fn is_rooted(&self) -> Option<bool> {
		self.is_rooted
	}

	pub fn tree(&self) -> &TreeBuilder {
		&self.tree
	}

	pub fn into_tree(self) -> TreeBuilder {
		self.tree
	}

	pub fn line(&self) -> usize {
		self.line
	}

	pub fn column(&self) -> usize {
		self.column
	}
}

#[derive(Debug)]
pub struct NexusTreeReader<R> {
	blocks: BlockReader<R>,
	block_index: Option<u64>,
	translation: Option<TranslationTable>,
	saw_tree: bool,
	finished: bool,
}

impl<R: BufRead> NexusTreeReader<R> {
	pub fn new(reader: R) -> Result<Self> {
		Ok(Self {
			blocks: BlockReader::new(reader)?,
			block_index: None,
			translation: None,
			saw_tree: false,
			finished: false,
		})
	}

	pub fn next_tree(&mut self) -> Result<Option<NexusTree>> {
		if self.finished {
			return Ok(None);
		}

		loop {
			let Some(command) = self.blocks.next_command()? else {
				self.finished = true;
				return Ok(None);
			};
			if !command.block().eq_ignore_ascii_case("trees") {
				continue;
			}
			if self.block_index != Some(command.block_index()) {
				self.block_index = Some(command.block_index());
				self.translation = None;
				self.saw_tree = false;
			}

			if command.name().eq_ignore_ascii_case("translate") {
				ensure!(
					!self.saw_tree,
					"TRANSLATE must appear before TREE commands at line {}, column {}",
					command.line(),
					command.column()
				);
				ensure!(
					self.translation.is_none(),
					"A TREES block contains more than one TRANSLATE command at line {}, column {}",
					command.line(),
					command.column()
				);
				self.translation = Some(
					TranslationTable::parse(&command)?,
				);
				continue;
			}

			if !command.name().eq_ignore_ascii_case("tree")
				&& !command.name().eq_ignore_ascii_case("utree")
			{
				continue;
			}

			self.saw_tree = true;
			let tree_command = TreeCommand::parse(&command)?;
			let mut tree = parse_newick(tree_command.newick())
				.with_context(|| {
					format!(
						"Could not parse NEXUS tree '{}' at line {}, column {}",
						tree_command.name(),
						tree_command.line(),
						tree_command.column()
					)
				})?;
			if let Some(translation) = &self.translation {
				translation.apply(&mut tree);
			}

			return Ok(Some(NexusTree {
				name: tree_command.name().to_owned(),
				is_default: tree_command.is_default(),
				is_rooted: tree_command.is_rooted(),
				tree,
				line: tree_command.line(),
				column: tree_command.column(),
			}));
		}
	}
}

impl<R: BufRead> Iterator for NexusTreeReader<R> {
	type Item = Result<NexusTree>;

	fn next(&mut self) -> Option<Self::Item> {
		match self.next_tree() {
			Ok(Some(tree)) => Some(Ok(tree)),
			Ok(None) => None,
			Err(error) => {
				self.finished = true;
				Some(Err(error))
			}
		}
	}
}

pub fn parse_trees(input: &str) -> Result<Vec<NexusTree>> {
	NexusTreeReader::new(Cursor::new(input))?.collect()
}

#[cfg(test)]
mod tests {
	use anyhow::Result;

	use super::parse_trees;

	#[test]
	fn parses_tree_blocks_into_builders() -> Result<()> {
		let source = "#NEXUS\nBEGIN TAXA; DIMENSIONS NTAX=3; END;\nBEGIN TREES;\nTRANSLATE 1 A, 2 'B B', 3 C;\nTREE * first = [&R] ((1:1,2:2):3,3:4);\nTITLE ignored;\nTREE second = (1:5,(2:6,3:7):8);\nEND;";
		let trees = parse_trees(source)?;
		assert_eq!(trees.len(), 2);
		assert_eq!(trees[0].name(), "first");
		assert!(trees[0].is_default());
		assert_eq!(trees[0].is_rooted(), Some(true));
		assert_eq!(
			trees[0].tree().to_newick()?,
			"((A:1,'B B':2):3,C:4)[&R];"
		);
		assert_eq!(trees[1].name(), "second");
		assert!(!trees[1].is_default());
		assert_eq!(trees[1].is_rooted(), None);
		assert_eq!(
			trees[1].tree().to_newick()?,
			"(A:5,('B B':6,C:7):8);"
		);
		Ok(())
	}

	#[test]
	fn resets_translation_between_tree_blocks() -> Result<()> {
		let source = "#NEXUS\nBEGIN TREES; TRANSLATE 1 A, 2 B; TREE first=(1:1,2:1); END;\nBEGIN NOTES; TEXT anything; END;\nBEGIN TREES; TREE second=(1:1,2:1); END;";
		let trees = parse_trees(source)?;
		assert_eq!(trees.len(), 2);
		assert_eq!(trees[0].tree().to_newick()?, "(A:1,B:1);");
		assert_eq!(trees[1].tree().to_newick()?, "(1:1,2:1);");
		Ok(())
	}

	#[test]
	fn supports_empty_and_unknown_blocks() -> Result<()> {
		let source = "#NEXUS\nBEGIN UNKNOWN; ANYTHING 'goes; here'; END; BEGIN TREES; END;";
		assert!(parse_trees(source)?.is_empty());
		Ok(())
	}

	#[test]
	fn reports_invalid_tree_context() {
		let source =
			"#NEXUS\nBEGIN TREES; TREE broken = (A:bad,B:1); END;";
		let error = format!("{:#}", parse_trees(source).unwrap_err());
		assert!(error
			.contains("NEXUS tree 'broken' at line 2, column 14"));
		assert!(error.contains("Invalid branch length 'bad'"));
	}

	#[test]
	fn rejects_invalid_translation_order() {
		for (source, message) in [
			(
				"#NEXUS\nBEGIN TREES; TREE first=(A:1,B:1); TRANSLATE A X, B Y; END;",
				"must appear before TREE",
			),
			(
				"#NEXUS\nBEGIN TREES; TRANSLATE A X, B Y; TRANSLATE A X, B Y; END;",
				"more than one TRANSLATE",
			),
		] {
			let error =
				parse_trees(source).unwrap_err().to_string();
			assert!(
				error.contains(message),
				"expected {message:?} in {error:?}"
			);
		}
	}
}
