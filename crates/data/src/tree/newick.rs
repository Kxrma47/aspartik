#![allow(unused)]

use anyhow::{Result, anyhow, bail};
use logos::{Lexer, Logos};
use smallvec::SmallVec;

use super::{Node, builder2::TreeBuilder2};

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

fn parse_newick<'a>(newick: &'a str) -> Result<TreeBuilder2> {
	let mut lexer = Token::lexer(newick);
	let mut tree = TreeBuilder2::new();

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
