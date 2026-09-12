use anyhow::{Context, Result, ensure};
use smallvec::SmallVec;

use std::{
	collections::{HashMap, hash_map::Entry},
	fmt::{self, Write},
};

use crate::tree::{BinaryTree, Node, builder::TreeBuilder};

#[derive(Clone, Copy)]
struct Edge<'a> {
	length: Option<f64>,
	attributes: &'a str,
}

#[derive(Clone, Copy)]
struct Child<'a> {
	node: Node,
	edge: Edge<'a>,
	hybrid: bool,
}

trait Source {
	fn validate(&self) -> Result<()>;
	fn root(&self) -> Node;
	fn children(&self, node: Node) -> Result<SmallVec<[Child<'_>; 2]>>;
	fn name(&self, node: Node) -> Option<&str>;
	fn node_attributes(&self, node: Node) -> Option<&str>;
}

impl Source for TreeBuilder {
	fn validate(&self) -> Result<()> {
		TreeBuilder::validate(self)
	}

	fn root(&self) -> Node {
		TreeBuilder::root(self)
	}

	fn children(&self, node: Node) -> Result<SmallVec<[Child<'_>; 2]>> {
		let mut children = SmallVec::new();
		for &child in self
			.children_of(node)
			.context("Node is out of range")?
		{
			let hybrid = self.parent_of(child) != Some(node);
			let edge = if hybrid {
				Edge {
					length: None,
					attributes: "",
				}
			} else {
				let edge = self.edge(child).context(
					"A canonical child has no edge data",
				)?;
				Edge {
					length: edge.length,
					attributes: &edge.attributes,
				}
			};
			children.push(Child {
				node: child,
				edge,
				hybrid,
			});
		}
		Ok(children)
	}

	fn name(&self, node: Node) -> Option<&str> {
		self.node(node).map(|data| data.name.as_str())
	}

	fn node_attributes(&self, node: Node) -> Option<&str> {
		self.node(node).map(|data| data.attributes.as_str())
	}
}

impl Source for BinaryTree {
	fn validate(&self) -> Result<()> {
		Ok(())
	}

	fn root(&self) -> Node {
		BinaryTree::root(self).into()
	}

	fn children(&self, node: Node) -> Result<SmallVec<[Child<'_>; 2]>> {
		let mut children = SmallVec::new();
		if let Some(internal) = self.as_internal(node) {
			let (left, right) = self.children_of(internal);
			for child in [left, right] {
				children.push(Child {
					node: child,
					edge: Edge {
						length: self.edge_length(child),
						attributes: self
							.edge_metadata(child)
							.unwrap_or_default(),
					},
					hybrid: false,
				});
			}
		}
		Ok(children)
	}

	fn name(&self, node: Node) -> Option<&str> {
		BinaryTree::name(self, node)
	}

	fn node_attributes(&self, node: Node) -> Option<&str> {
		self.node_metadata(node)
	}
}

enum Event<'a> {
	Subtree(Child<'a>),
	Close(Node, Option<Edge<'a>>),
	Comma,
}

impl TreeBuilder {
	pub fn write_newick<W: Write>(&self, writer: &mut W) -> Result<()> {
		write_newick(self, writer)
	}

	pub fn to_newick(&self) -> Result<String> {
		to_newick(self)
	}
}

impl BinaryTree {
	pub fn write_newick<W: Write>(&self, writer: &mut W) -> Result<()> {
		write_newick(self, writer)
	}

	pub fn to_newick(&self) -> Result<String> {
		to_newick(self)
	}
}

fn to_newick(source: &impl Source) -> Result<String> {
	let mut output = String::new();
	write_newick(source, &mut output)?;
	Ok(output)
}

fn write_newick<W: Write>(source: &impl Source, writer: &mut W) -> Result<()> {
	source.validate()?;
	let root = source.root();
	let mut stack = vec![Event::Subtree(Child {
		node: root,
		edge: Edge {
			length: None,
			attributes: "",
		},
		hybrid: false,
	})];
	let mut defined_hybrids = HashMap::new();

	while let Some(event) = stack.pop() {
		match event {
			Event::Comma => writer.write_char(',')?,
			Event::Close(node, edge) => {
				writer.write_char(')')?;
				write_node(source, node, edge, writer)?;
			}
			Event::Subtree(child) => {
				let name = source
					.name(child.node)
					.unwrap_or_default();
				let hybrid = hybrid_identifier(name);
				if child.hybrid {
					ensure!(
						hybrid.is_some(),
						"A hybrid edge target must have a hybrid identifier"
					);
				}
				if let Some(identifier) = hybrid {
					match defined_hybrids.entry(identifier) {
						Entry::Vacant(entry) => {
							entry.insert(child.node);
						}
						Entry::Occupied(entry)
							if *entry.get() == child.node =>
						{
							write_label(name, writer)?;
							write_edge(child.edge, writer)?;
							continue;
						}
						Entry::Occupied(_) => {
							anyhow::bail!(
								"Hybrid identifier '#{identifier}' is used by more than one node"
							)
						}
					}
				}

				let children = source.children(child.node)?;
				if children.is_empty() {
					write_node(
						source,
						child.node,
						(child.node != root)
							.then_some(child.edge),
						writer,
					)?;
					continue;
				}

				writer.write_char('(')?;
				stack.push(Event::Close(
					child.node,
					(child.node != root)
						.then_some(child.edge),
				));
				for (index, next) in
					children.into_iter().enumerate().rev()
				{
					stack.push(Event::Subtree(next));
					if index > 0 {
						stack.push(Event::Comma);
					}
				}
			}
		}
	}
	writer.write_char(';')?;
	Ok(())
}

fn write_node<W: Write>(
	source: &impl Source,
	node: Node,
	edge: Option<Edge<'_>>,
	writer: &mut W,
) -> Result<()> {
	let name = source.name(node).unwrap_or_default();
	write_label(name, writer)?;
	writer.write_str(source.node_attributes(node).unwrap_or_default())?;
	if let Some(edge) = edge {
		write_edge(edge, writer)?;
	}
	Ok(())
}

fn write_edge<W: Write>(edge: Edge<'_>, writer: &mut W) -> fmt::Result {
	if edge.length.is_some() || !edge.attributes.is_empty() {
		writer.write_char(':')?;
	}
	if let Some(length) = edge.length {
		write!(writer, "{length}")?;
	}
	writer.write_str(edge.attributes)
}

fn write_label<W: Write>(label: &str, writer: &mut W) -> fmt::Result {
	if label.is_empty() {
		return Ok(());
	}
	if !label.chars().any(|character| {
		character.is_whitespace()
			|| matches!(
				character,
				'(' | ')'
					| '[' | ']' | ',' | ':' | ';' | '\''
					| '"'
			)
	}) {
		return writer.write_str(label);
	}

	writer.write_char('\'')?;
	for character in label.chars() {
		writer.write_char(character)?;
		if character == '\'' {
			writer.write_char('\'')?;
		}
	}
	writer.write_char('\'')
}

fn hybrid_identifier(name: &str) -> Option<&str> {
	let identifier = name.rsplit_once('#')?.1;
	(!identifier.is_empty()).then_some(identifier)
}
