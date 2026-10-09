use anyhow::Result;
use arbitrary::Unstructured;
use arbtest::arbtest;

use std::io::{BufReader, Cursor};

use data::{TaxonSet, nexus::for_each_tree, tree::BinaryTree};

const BEAST: &str = "#NEXUS\nBEGIN TAXA; TAXLABELS A B 'C C'; END;\nBEGIN TREES;\nTRANSLATE 2 B, 3 'C C', 1 A;\nTREE STATE_0 = [&R] ((1[&date=2020]:0.1[&rate=0.5],2:0.2)[&posterior=0.9]:0.3,3:0.4);\nTREE STATE_10000 = [&R] (1:0.2,(2:0.3,3:0.4):0.5);\nEND;";

#[test]
fn translated_trees_share_taxa() -> Result<()> {
	let mut trees = Vec::new();
	for_each_tree(Cursor::new(BEAST), |newick, translation| {
		let (aliases, taxa) = translation.unwrap();
		let tree = BinaryTree::parse_newick_with_translation(
			newick,
			aliases,
			taxa.clone(),
		)?;
		assert_eq!(taxa.iter().collect::<Vec<_>>(), ["A", "B", "C C"]);
		trees.push(tree);
		Ok(())
	})?;
	assert_eq!(trees.len(), 2);
	assert!(std::ptr::eq(trees[0].taxa().get(0), trees[1].taxa().get(0)));
	assert_eq!(
		trees[0].to_newick()?,
		"((A[&date=2020]:0.1[&rate=0.5],B:0.2)[&posterior=0.9]:0.3,'C C':0.4)[&R];"
	);
	assert_eq!(trees[1].to_newick()?, "(A:0.2,(B:0.3,'C C':0.4):0.5)[&R];");
	Ok(())
}

#[test]
fn untranslated_trees_can_use_an_existing_taxon_set() -> Result<()> {
	let taxa = TaxonSet::from_iter(["A", "B", "C"]);
	let source = "#NEXUS\nBEGIN TREES; TREE first = ((B:2,A:1):3,C:4); TREE second = ((A:1,B:2):3,C:4); END;";
	let mut trees = Vec::new();
	for_each_tree(Cursor::new(source), |newick, translation| {
		assert!(translation.is_none());
		trees.push(BinaryTree::parse_newick_with_taxa(
			newick,
			taxa.clone(),
		)?);
		Ok(())
	})?;
	assert_eq!(trees.len(), 2);
	assert!(trees[0].identical_children(&trees[1]));
	Ok(())
}

#[test]
fn works_with_small_reader_buffers() -> Result<()> {
	for capacity in 1..=16 {
		let reader =
			BufReader::with_capacity(capacity, Cursor::new(BEAST));
		let mut count = 0;
		for_each_tree(reader, |newick, translation| {
			let (aliases, taxa) = translation.unwrap();
			BinaryTree::parse_newick_with_translation(
				newick,
				aliases,
				taxa.clone(),
			)?;
			count += 1;
			Ok(())
		})?;
		assert_eq!(count, 2);
	}
	Ok(())
}

#[test]
fn streams_ten_thousand_trees() -> Result<()> {
	const COUNT: usize = 10_000;
	let mut source =
		String::from("#NEXUS\nBEGIN TREES; TRANSLATE 2 B, 1 A;\n");
	for index in 0..COUNT {
		source.push_str(&format!("TREE STATE_{index} = (1:1,2:2);\n"));
	}
	source.push_str("END;\n");
	let mut count = 0;
	for_each_tree(Cursor::new(source), |newick, translation| {
		let (aliases, taxa) = translation.unwrap();
		let tree = BinaryTree::parse_newick_with_translation(
			newick,
			aliases,
			taxa.clone(),
		)?;
		assert_eq!(tree.num_leaves(), 2);
		count += 1;
		Ok(())
	})?;
	assert_eq!(count, COUNT);
	Ok(())
}

#[test]
fn reports_unknown_translated_leaf() {
	let source = "#NEXUS\nBEGIN TREES; TRANSLATE 1 A, 2 B; TREE bad = (1:1,3:2); END;";
	let error =
		for_each_tree(Cursor::new(source), |newick, translation| {
			let (aliases, taxa) = translation.unwrap();
			BinaryTree::parse_newick_with_translation(
				newick,
				aliases,
				taxa.clone(),
			)?;
			Ok(())
		})
		.unwrap_err();
	assert!(error.to_string().contains("Unknown taxon: 3"));
}

#[test]
fn arbitrary_input_does_not_panic() {
	arbtest(|u: &mut Unstructured<'_>| {
		let len = u.int_in_range(0_usize..=1024)?;
		let bytes = u.bytes(len)?;
		let mut source = String::from("#NEXUS\n");
		source.extend(bytes
			.iter()
			.map(|byte| char::from(32 + byte % 95)));
		let _ = for_each_tree(Cursor::new(source), |_, _| Ok(()));
		Ok(())
	})
	.size_min(2_u32.pow(18));
}
