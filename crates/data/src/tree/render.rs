use anyhow::{Result, ensure};

use std::fmt::{self, Write};

use super::{BinaryTree, Node};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
	pub x: f64,
	pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
	Rectangular,
	Tidy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeLayout {
	coordinates: Box<[Point]>,
	width: f64,
	height: f64,
	kind: LayoutKind,
}

impl TreeLayout {
	pub fn point(&self, node: Node) -> Option<Point> {
		self.coordinates.get(node.index() as usize).copied()
	}

	pub fn points(&self) -> &[Point] {
		&self.coordinates
	}

	pub fn width(&self) -> f64 {
		self.width
	}

	pub fn height(&self) -> f64 {
		self.height
	}

	pub fn kind(&self) -> LayoutKind {
		self.kind
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgOptions {
	pub x_scale: f64,
	pub y_scale: f64,
	pub margin: f64,
	pub node_radius: f64,
	pub font_size: f64,
}

impl Default for SvgOptions {
	fn default() -> Self {
		Self {
			x_scale: 100.0,
			y_scale: 30.0,
			margin: 20.0,
			node_radius: 3.0,
			font_size: 12.0,
		}
	}
}

#[derive(Debug, Clone, Copy)]
struct TidyNode {
	preliminary: f64,
	modifier: f64,
	left_thread: Option<Node>,
	right_thread: Option<Node>,
	extreme_left: Node,
	extreme_right: Node,
	modifier_extreme_left: f64,
	modifier_extreme_right: f64,
}

impl BinaryTree {
	pub fn rectangular_layout(
		&self,
		separation: f64,
	) -> Result<TreeLayout> {
		let distances = self.layout_distances(separation)?;
		let mut coordinates = vec![
			Point { x: 0.0, y: 0.0 };
			self.num_nodes() as usize
		];
		let mut next_leaf = 0_u32;
		for node in self.preorder() {
			coordinates[node.index() as usize].x =
				distances[node.index() as usize];
			if self.is_leaf(node) {
				coordinates[node.index() as usize].y =
					f64::from(next_leaf) * separation;
				next_leaf += 1;
			}
		}
		for node in self.postorder() {
			let Some(internal) = self.as_internal(node) else {
				continue;
			};
			let (left, right) = self.children_of(internal);
			coordinates[node.index() as usize].y =
				(coordinates[left.index() as usize].y
					+ coordinates[right.index() as usize]
						.y) / 2.0;
		}
		Ok(TreeLayout {
			width: distances.into_iter().fold(0.0, f64::max),
			height: f64::from(next_leaf.saturating_sub(1))
				* separation,
			coordinates: coordinates.into_boxed_slice(),
			kind: LayoutKind::Rectangular,
		})
	}

	pub fn tidy_layout(&self, separation: f64) -> Result<TreeLayout> {
		let distances = self.layout_distances(separation)?;
		let mut state = self
			.nodes()
			.map(|node| TidyNode {
				preliminary: 0.0,
				modifier: 0.0,
				left_thread: None,
				right_thread: None,
				extreme_left: node,
				extreme_right: node,
				modifier_extreme_left: 0.0,
				modifier_extreme_right: 0.0,
			})
			.collect::<Vec<_>>();

		for node in self.postorder() {
			let Some(internal) = self.as_internal(node) else {
				continue;
			};
			let (left, right) = self.children_of(internal);
			separate_subtrees(
				self, left, right, &distances, &mut state,
				separation,
			);
			state[node.index() as usize].preliminary =
				(state[left.index() as usize].preliminary
					+ state[left.index() as usize]
						.modifier + state[right.index() as usize]
					.modifier + state[right.index() as usize]
					.preliminary) / 2.0;
			state[node.index() as usize].extreme_left =
				state[left.index() as usize].extreme_left;
			state[node.index() as usize].modifier_extreme_left =
				state[left.index() as usize]
					.modifier_extreme_left;
			state[node.index() as usize].extreme_right =
				state[right.index() as usize].extreme_right;
			state[node.index() as usize].modifier_extreme_right =
				state[right.index() as usize]
					.modifier_extreme_right;
		}

		let mut vertical = vec![0.0; self.num_nodes() as usize];
		let mut minimum = f64::INFINITY;
		let mut maximum = f64::NEG_INFINITY;
		let mut stack = vec![(Node::from(self.root()), 0.0)];
		while let Some((node, modifier_sum)) = stack.pop() {
			let item = state[node.index() as usize];
			let modifier_sum = modifier_sum + item.modifier;
			let value = item.preliminary + modifier_sum;
			vertical[node.index() as usize] = value;
			minimum = minimum.min(value);
			maximum = maximum.max(value);
			if let Some(internal) = self.as_internal(node) {
				let (left, right) = self.children_of(internal);
				stack.push((right, modifier_sum));
				stack.push((left, modifier_sum));
			}
		}

		let coordinates = self
			.nodes()
			.map(|node| Point {
				x: distances[node.index() as usize],
				y: vertical[node.index() as usize] - minimum,
			})
			.collect::<Vec<_>>();
		Ok(TreeLayout {
			width: distances.into_iter().fold(0.0, f64::max),
			height: maximum - minimum,
			coordinates: coordinates.into_boxed_slice(),
			kind: LayoutKind::Tidy,
		})
	}

	pub fn to_svg<NC, EC, N, E>(
		&self,
		layout: &TreeLayout,
		options: SvgOptions,
		node_color: NC,
		edge_color: EC,
	) -> Result<String>
	where
		NC: Fn(Node) -> N,
		EC: Fn(Node) -> E,
		N: AsRef<str>,
		E: AsRef<str>,
	{
		ensure!(
			layout.coordinates.len() == self.num_nodes() as usize,
			"The layout does not match the tree"
		);
		validate_svg_options(options)?;

		let scaled = |point: Point| Point {
			x: options.margin + point.x * options.x_scale,
			y: options.margin + point.y * options.y_scale,
		};
		let label_gap = options.node_radius + options.font_size * 0.5;
		let mut content_width = layout.width * options.x_scale;
		for leaf in self.leaves() {
			let node = Node::from(leaf);
			let Some(name) = self.name(node) else {
				continue;
			};
			let x = layout.coordinates[node.index() as usize].x
				* options.x_scale;
			content_width = content_width.max(x
				+ label_gap + options
				.font_size
				* 0.6
				* name.chars().count() as f64);
		}
		let width = content_width + options.margin * 2.0;
		let height =
			layout.height * options.y_scale + options.margin * 2.0;
		let mut output = String::new();
		write!(
			output,
			"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
		)?;

		for child in self.edges() {
			let parent = self.parent_of(child).unwrap();
			let parent_point =
				scaled(layout.coordinates
					[parent.index() as usize]);
			let child_point =
				scaled(layout.coordinates
					[child.index() as usize]);
			let color = edge_color(child);
			match layout.kind {
				LayoutKind::Rectangular => write!(
					output,
					"<path d=\"M {} {} V {} H {}\" fill=\"none\" stroke=\"",
					parent_point.x,
					parent_point.y,
					child_point.y,
					child_point.x
				)?,
				LayoutKind::Tidy => write!(
					output,
					"<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"",
					parent_point.x,
					parent_point.y,
					child_point.x,
					child_point.y
				)?,
			}
			write_escaped(&mut output, color.as_ref())?;
			output.push_str("\">");
			if let Some(metadata) = self.edge_metadata(child) {
				output.push_str("<title>");
				write_escaped(&mut output, metadata)?;
				output.push_str("</title>");
			}
			match layout.kind {
				LayoutKind::Rectangular => {
					output.push_str("</path>")
				}
				LayoutKind::Tidy => output.push_str("</line>"),
			}
		}

		for node in self.nodes() {
			let point =
				scaled(layout.coordinates
					[node.index() as usize]);
			let color = node_color(node);
			write!(
				output,
				"<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"",
				point.x, point.y, options.node_radius
			)?;
			write_escaped(&mut output, color.as_ref())?;
			output.push_str("\">");
			if self.name(node).is_some()
				|| self.node_metadata(node).is_some()
			{
				output.push_str("<title>");
				if let Some(name) = self.name(node) {
					write_escaped(&mut output, name)?;
				}
				if let Some(metadata) = self.node_metadata(node)
				{
					if self.name(node).is_some() {
						output.push(' ');
					}
					write_escaped(&mut output, metadata)?;
				}
				output.push_str("</title>");
			}
			output.push_str("</circle>");
		}

		for leaf in self.leaves() {
			let node = Node::from(leaf);
			let Some(name) = self.name(node) else {
				continue;
			};
			let point =
				scaled(layout.coordinates
					[node.index() as usize]);
			write!(
				output,
				"<text x=\"{}\" y=\"{}\" dominant-baseline=\"middle\" font-size=\"{}\">",
				point.x + label_gap,
				point.y,
				options.font_size
			)?;
			write_escaped(&mut output, name)?;
			output.push_str("</text>");
		}
		output.push_str("</svg>");
		Ok(output)
	}

	fn layout_distances(&self, separation: f64) -> Result<Vec<f64>> {
		ensure!(
			separation.is_finite() && separation > 0.0,
			"Node separation must be finite and positive"
		);
		let mut distances = vec![0.0; self.num_nodes() as usize];
		for node in self.preorder() {
			let Some(internal) = self.as_internal(node) else {
				continue;
			};
			let (left, right) = self.children_of(internal);
			for child in [left, right] {
				let length = self.edge_length(child).unwrap();
				ensure!(
					length.is_finite() && length >= 0.0,
					"Branch length for node {} must be finite and nonnegative",
					child.index()
				);
				distances[child.index() as usize] = distances
					[node.index() as usize]
					+ length;
			}
		}
		Ok(distances)
	}
}

fn separate_subtrees(
	tree: &BinaryTree,
	left: Node,
	right: Node,
	distances: &[f64],
	state: &mut [TidyNode],
	separation: f64,
) {
	let mut upper = Some(left);
	let mut lower = Some(right);
	let mut upper_modifiers = state[left.index() as usize].modifier;
	let mut lower_modifiers = state[right.index() as usize].modifier;
	let mut first = true;

	while let (Some(upper_node), Some(lower_node)) = (upper, lower) {
		let distance = upper_modifiers
			+ state[upper_node.index() as usize].preliminary
			+ separation - lower_modifiers
			- state[lower_node.index() as usize].preliminary;
		if (first && distance < 0.0) || distance > 0.0 {
			lower_modifiers += distance;
			let item = &mut state[right.index() as usize];
			item.modifier += distance;
			item.modifier_extreme_left += distance;
			item.modifier_extreme_right += distance;
			first = false;
		}

		let upper_depth = distances[upper_node.index() as usize];
		let lower_depth = distances[lower_node.index() as usize];
		if upper_depth <= lower_depth {
			upper = next_right_contour(tree, state, upper_node);
			if let Some(node) = upper {
				upper_modifiers +=
					state[node.index() as usize].modifier;
			}
		}
		if upper_depth >= lower_depth {
			lower = next_left_contour(tree, state, lower_node);
			if let Some(node) = lower {
				lower_modifiers +=
					state[node.index() as usize].modifier;
			}
		}
	}

	match (upper, lower) {
		(None, Some(lower_node)) => {
			let extreme = state[left.index() as usize].extreme_left;
			state[extreme.index() as usize].left_thread =
				Some(lower_node);
			let difference = lower_modifiers
				- state[lower_node.index() as usize].modifier
				- state[left.index() as usize]
					.modifier_extreme_left;
			state[extreme.index() as usize].modifier += difference;
			state[extreme.index() as usize].preliminary -=
				difference;
			state[left.index() as usize].extreme_left =
				state[right.index() as usize].extreme_left;
			state[left.index() as usize].modifier_extreme_left =
				state[right.index() as usize]
					.modifier_extreme_left;
		}
		(Some(upper_node), None) => {
			let extreme =
				state[right.index() as usize].extreme_right;
			state[extreme.index() as usize].right_thread =
				Some(upper_node);
			let difference = upper_modifiers
				- state[upper_node.index() as usize].modifier
				- state[right.index() as usize]
					.modifier_extreme_right;
			state[extreme.index() as usize].modifier += difference;
			state[extreme.index() as usize].preliminary -=
				difference;
			state[right.index() as usize].extreme_right =
				state[left.index() as usize].extreme_right;
			state[right.index() as usize].modifier_extreme_right =
				state[left.index() as usize]
					.modifier_extreme_right;
		}
		_ => {}
	}
}

fn next_left_contour(
	tree: &BinaryTree,
	state: &[TidyNode],
	node: Node,
) -> Option<Node> {
	if let Some(internal) = tree.as_internal(node) {
		Some(tree.children_of(internal).0)
	} else {
		state[node.index() as usize].left_thread
	}
}

fn next_right_contour(
	tree: &BinaryTree,
	state: &[TidyNode],
	node: Node,
) -> Option<Node> {
	if let Some(internal) = tree.as_internal(node) {
		Some(tree.children_of(internal).1)
	} else {
		state[node.index() as usize].right_thread
	}
}

fn validate_svg_options(options: SvgOptions) -> Result<()> {
	for (name, value) in [
		("Horizontal scale", options.x_scale),
		("Vertical scale", options.y_scale),
		("Node radius", options.node_radius),
		("Font size", options.font_size),
	] {
		ensure!(
			value.is_finite() && value > 0.0,
			"{name} must be finite and positive"
		);
	}
	ensure!(
		options.margin.is_finite() && options.margin >= 0.0,
		"Margin must be finite and nonnegative"
	);
	Ok(())
}

fn write_escaped(writer: &mut impl fmt::Write, value: &str) -> fmt::Result {
	for character in value.chars() {
		match character {
			'&' => writer.write_str("&amp;")?,
			'<' => writer.write_str("&lt;")?,
			'>' => writer.write_str("&gt;")?,
			'\"' => writer.write_str("&quot;")?,
			'\'' => writer.write_str("&apos;")?,
			_ => writer.write_char(character)?,
		}
	}
	Ok(())
}
