#![allow(unused)]

use anyhow::{Result, anyhow, bail};
use logos::{Lexer, Logos};
use smallvec::SmallVec;

use std::iter::Peekable;

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
		self.parents.push(Node::tombstone());
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

fn peek<'a>(lexer: &mut Peekable<Lexer<'a, Token<'a>>>) -> Result<Token<'a>> {
	let Some(token) = lexer.peek() else {
		bail!("Unexpected end of input");
	};
	let Ok(token) = token else {
		bail!("Couldn't read next token");
	};
	Ok(*token)
}

fn advance<'a>(
	lexer: &mut Peekable<Lexer<'a, Token<'a>>>,
	token: &mut Token<'a>,
) -> Result<()> {
	lexer.next();
	*token = peek(lexer)?;
	println!("advance to {token:?}");
	Ok(())
}

fn parse_node<'a>(
	lexer: &mut Peekable<Lexer<'a, Token<'a>>>,
	token: &mut Token<'a>,
	tree: &mut TreeBuilder,
	node: Node,
) -> Result<Node> {
	if let Token::Ident(name) | Token::Quoted(name) = token {
		tree.set_node_name(node, name);
		advance(lexer, token);
	}

	if let Token::Attribute(attr) = token {
		tree.set_node_attribute(node, attr);
		advance(lexer, token);
	}

	if let Token::Colon = token {
		advance(lexer, token);
		if let Token::Ident(s) = token {
			tree.set_edge_length(node, s.parse::<f64>()?);
			advance(lexer, token);
		}

		if let Token::Attribute(attr) = token {
			tree.set_edge_attribute(node, attr);
			advance(lexer, token);
		}
	}

	Ok(node)
}

fn parse_newick(newick: &str) -> Result<TreeBuilder> {
	let mut lexer = Token::lexer(newick).peekable();
	let mut tree = TreeBuilder::new();

	let mut token = peek(&mut lexer)?;

	if token != Token::Open {
		bail!("TODO error");
	}

	let mut children = Vec::<SmallVec<[Node; 2]>>::new();
	let mut parents = Vec::<Node>::new();

	loop {
		match token {
			Token::Open => {
				advance(&mut lexer, &mut token);

				parents.push(tree.add_dummy_node());
				children.push(SmallVec::new());
				let node = tree.add_dummy_node();
				parse_node(
					&mut lexer, &mut token, &mut tree, node,
				);
				children.last_mut().unwrap().push(node);
			}
			Token::Comma => {
				advance(&mut lexer, &mut token);

				if token == Token::Open {
					continue;
				}
				let node = tree.add_dummy_node();
				parse_node(
					&mut lexer, &mut token, &mut tree, node,
				);
				children.last_mut().unwrap().push(node);
			}
			Token::Close => {
				advance(&mut lexer, &mut token);

				let parent = parents.pop().unwrap();
				parse_node(
					&mut lexer, &mut token, &mut tree,
					parent,
				);
				let children = children.pop().unwrap();
				tree.set_node_children(parent, &children);
				for child in children {
					tree.set_node_parent(child, parent);
				}
			}
			Token::Semicolon => {
				advance(&mut lexer, &mut token);

				if !parents.is_empty() {
					bail!("early termination");
				}
				if !lexer.next().is_none() {
					bail!("trailing characters");
				}
				break;
			}

			t => bail!("Unexpected token: {t:?}"),
		}
	}

	Ok(tree)
}
