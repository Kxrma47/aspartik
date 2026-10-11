#![allow(unused)]

use anyhow::{Result, anyhow, bail};
use logos::{Lexer, Logos};
use smallvec::SmallVec;

use super::Node;

#[derive(Debug, Clone)]
struct TreeBuilder {
	parents: Vec<Node>,
	children: Vec<SmallVec<[Node; 2]>>,
	node_names: Vec<String>,
	node_attributes: Vec<String>,
	edge_lengths: Vec<f64>,
	edge_attributes: Vec<String>,
	root: Option<Node>,
}

impl TreeBuilder {
	fn new() -> Self {
		Self {
			parents: Vec::new(),
			children: Vec::new(),
			node_names: Vec::new(),
			node_attributes: Vec::new(),
			edge_lengths: Vec::new(),
			edge_attributes: Vec::new(),
			root: None,
		}
	}

	fn add_dummy_node(&mut self) -> Node {
		let idx = self.parents.len() as u32;
		self.parents.push(Node::TOMBSTONE);
		self.children.push(SmallVec::new());
		self.node_names.push(String::new());
		self.node_attributes.push(String::new());
		self.edge_lengths.push(f64::NAN);
		self.edge_attributes.push(String::new());

		Node(idx)
	}

	fn add_node(
		&mut self,
		name: &str,
		edge_length: f64,
		parent: Node,
	) -> Node {
		let idx = self.parents.len() as u32;
		let new = Node(idx);

		self.parents.push(parent);
		self.children.push(SmallVec::new());
		self.node_names.push(name.to_owned());
		self.node_attributes.push(String::new());
		self.edge_lengths.push(edge_length);
		self.edge_attributes.push(String::new());

		self.children[parent.usize()].push(new);

		new
	}

	fn set_root(&mut self, node: Node) {
		self.root = Some(node);
	}

	// Node setters
	fn set_node_parent(&mut self, node: Node, parent: Node) {
		self.parents[node.usize()] = parent;
	}

	fn set_node_children(&mut self, node: Node, children: &[Node]) {
		self.children[node.usize()] = SmallVec::from_slice(children);
	}

	fn set_node_name(&mut self, node: Node, name: &str) {
		self.node_names[node.usize()].replace_range(.., name);
	}

	fn set_node_attribute(&mut self, node: Node, attr: &str) {
		self.node_attributes[node.usize()].replace_range(.., attr);
	}

	fn set_edge_length(&mut self, node: Node, length: f64) {
		self.edge_lengths[node.usize()] = length;
	}

	fn set_edge_attribute(&mut self, node: Node, attr: &str) {
		self.edge_attributes[node.usize()].replace_range(.., attr);
	}

	// Node getters
	fn parent(&self, node: Node) -> Node {
		self.parents[node.usize()]
	}

	fn children(&self, node: Node) -> &[Node] {
		&self.children[node.usize()]
	}

	fn node_name(&self, node: Node) -> &str {
		&self.node_names[node.usize()]
	}

	fn node_attribute(&self, node: Node) -> &str {
		&self.node_attributes[node.usize()]
	}

	fn edge_length(&self, node: Node) -> Option<f64> {
		let length = self.edge_lengths[node.usize()];
		if length.is_nan() { None } else { Some(length) }
	}

	fn edge_attribute(&self, node: Node) -> &str {
		&self.edge_attributes[node.usize()]
	}

	fn root(&self) -> Option<Node> {
		self.root
	}
}

fn unquote(s: &str) -> &str {
	&s[1..s.len() - 1]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Logos)]
#[logos(skip(r"[ \t\r\n]+"))]
enum Token<'a> {
	#[token("(")]
	Open,
	#[token(")")]
	Close,
	#[token(",")]
	Comma,
	#[token(":")]
	Colon,
	#[token(";")]
	Semicolon,
	#[regex(r"[^ \[\t\r\n():,;]+", |lex| lex.slice())]
	Ident(&'a str),
	#[regex(r#""[^"]*""#, |lex| unquote(lex.slice()))]
	#[regex(r"'[^']*'", |lex| unquote(lex.slice()))]
	Quoted(&'a str),
	#[regex(r"\[[^\]]*\]", |lex| unquote(lex.slice()))]
	Attribute(&'a str),
}

fn parse_newick<'a>(newick: &'a str) -> Result<TreeBuilder> {
	let mut lexer = Token::lexer(newick);
	let mut tree = TreeBuilder::new();

	let mut next = |lexer: &mut Lexer<'a, Token<'a>>| -> Result<Token<'a>> {
		match lexer.next() {
			Some(Ok(t)) => Ok(t),
			Some(Err(_)) => bail!("Lexing error"),
			None => bail!("Unexpected end of input"),
		}
	};

	let mut token = next(&mut lexer)?;
	if token != Token::Open {
		bail!("Expected a Newick tree to start with '('");
	}

	let mut parents = Vec::new();
	let mut children = Vec::new();

	let root = tree.add_dummy_node();
	parents.push(root);
	children.push(Vec::new());
	let mut node = tree.add_dummy_node();

	loop {
		if let Token::Ident(name) | Token::Quoted(name) = token {
			tree.set_node_name(node, name);
			token = next(&mut lexer)?;
		};

		if let Token::Attribute(attr) = token {
			tree.set_node_attribute(node, attr);
			token = next(&mut lexer)?;
		}

		if token == Token::Colon {
			token = next(&mut lexer)?;
			if let Token::Ident(s) = token {
				tree.set_edge_length(node, s.parse::<f64>()?);
				token = next(&mut lexer)?;
			} else {
				bail!("Expected edge length");
			}

			if let Token::Attribute(attr) = token {
				tree.set_edge_attribute(node, attr);
				token = next(&mut lexer)?;
			}
		}

		match token {
			Token::Open => {
				parents.push(node);
				children.push(Vec::new());
				token = next(&mut lexer)?;
				node = tree.add_dummy_node();
			}
			Token::Comma => {
				children.last_mut().unwrap().push(node);
				token = next(&mut lexer)?;
				node = tree.add_dummy_node();
			}
			Token::Close => {
				children.last_mut().unwrap().push(node);
				let parent = parents.pop().unwrap();
				let node_children = children.pop().unwrap();
				tree.set_node_children(parent, &node_children);
				for &child in &node_children {
					tree.set_node_parent(child, parent);
				}
				node = parent;
				token = next(&mut lexer)?;
			}
			Token::Semicolon => {
				if !parents.is_empty() {
					bail!("Unclosed parentheses");
				}
				if lexer.next().is_some() {
					bail!("Trailing characters");
				}
				break;
			}
			t => bail!("Unexpected token: {t:?}"),
		}
	}

	tree.set_root(root);

	Ok(tree)
}
