import pytest

from typing import Any, cast

from aspartik.data.tree import BinaryTree, Internal, Leaf, Node, TreeBuilder


def test_empty_tree():
    assert str(TreeBuilder()) == ";"


def test_from_newick():
    assert str(TreeBuilder.from_newick("(A:1,B:2)root;")) == "(A:1,B:2)root;"


def node_named(tree, name):
    return next(node for node in tree.nodes() if tree.name(node) == name)


def test_tree_access_and_traversal():
    tree = TreeBuilder.from_newick("((A:1,B:2)left:3,C:4)root;")
    root = tree.root
    left = node_named(tree, "left")
    a = node_named(tree, "A")

    assert isinstance(root, Node)
    assert all(isinstance(node, Node) for node in tree.nodes())
    assert isinstance(tree.parent_of(a), Node)
    assert root.index == int(root)
    assert len(tree) == tree.num_nodes == 5
    assert [node.index for node in tree.nodes()] == list(range(5))
    assert tree.children_of(root) == [left, node_named(tree, "C")]
    assert tree.parent_of(root) is None
    assert tree.parent_of(a) == left
    assert tree.is_leaf(a)
    assert not tree.is_leaf(left)
    assert tree.is_binary()
    assert tree.edge_length(a) == 1.0
    assert tree.preorder()[0] == root
    assert tree.postorder()[-1] == root
    assert sorted(tree.preorder()) == tree.nodes()
    assert sorted(tree.postorder()) == tree.nodes()


def test_tree_mutation_and_sealing():
    tree = TreeBuilder()
    tree.set_name(tree.root, "root")
    left = tree.add_node(tree.root, "left", 1.0)
    tree.add_node(tree.root, "right", 2.0)
    a = tree.add_node(left, "A", 3.0)
    b = tree.add_node(left, "B", 4.0)

    tree.set_node_metadata(a, "[&date=2020]")
    tree.set_edge_metadata(a, "[&rate=0.5]")
    tree.set_edge_length(a, 3.5)
    assert tree.node_metadata(a) == "[&date=2020]"
    assert tree.edge_metadata(a) == "[&rate=0.5]"
    assert tree.edge_length(a) == 3.5

    tree.replace_parent(b, tree.root)
    assert not tree.is_binary()
    tree.replace_parent(b, left)
    length, metadata = tree.remove_edge(left, b)
    assert (length, metadata) == (4.0, None)
    tree.add_edge(left, b, length, metadata)
    tree.validate()

    binary = tree.to_binary()
    assert isinstance(binary, BinaryTree)
    assert str(binary) == "((A[&date=2020]:3.5[&rate=0.5],B:4)left:1,right:2)root;"


def test_rerooting():
    tree = TreeBuilder.from_newick("((A:1,B:2)left:3,(C:4,D:5)right:6)root;")
    old_root = tree.root
    left = node_named(tree, "left")

    tree.set_root(left)
    assert tree.root == left
    assert tree.parent_of(old_root) == left
    assert tree.edge_length(old_root) == 3.0
    tree.validate()

    tree.set_root(old_root)
    assert tree.root == old_root
    assert tree.to_newick() == "((C:4,D:5)right:6,(A:1,B:2)left:3)root;"


def test_hybrid_children_and_canonical_traversal():
    tree = TreeBuilder.from_newick("((A:1,(C:1)X#H1:2)L:1,(X#H1:3,B:1)R:1)root;")
    hybrid = node_named(tree, "X#H1")
    additional_parent = node_named(tree, "R")

    assert hybrid in tree.children_of(additional_parent)
    assert tree.parent_of(hybrid) != additional_parent
    assert tree.preorder().count(hybrid) == 1
    assert tree.postorder().count(hybrid) == 1
    tree.remove_hybrid_edge(additional_parent, hybrid)
    assert hybrid not in tree.children_of(additional_parent)
    tree.add_hybrid_edge(additional_parent, hybrid)
    tree.validate()


def test_binary_tree_access():
    tree = BinaryTree.from_newick(
        "((A[&&NHX:S=human]:1[&rate=0.5],B:2)left:3,C:4)root;"
    )
    root = tree.root
    a = tree.leaf_by_name("A")
    left = next(node for node in tree.internals() if tree.name(node) == "left")

    assert a is not None
    assert isinstance(root, Internal)
    assert isinstance(a, Leaf)
    assert isinstance(left, Internal)
    assert all(isinstance(node, Node) for node in tree.nodes())
    assert root == tree.nodes()[root.index]
    assert root != root.index
    assert len(tree) == tree.num_nodes == 5
    assert tree.num_leaves == 3
    assert tree.num_internals == 2
    assert tree.num_edges == 4
    assert [node.index for node in tree.nodes()] == list(range(5))
    assert [leaf.index for leaf in tree.leaves()] == [0, 1, 2]
    assert {node.index for node in tree.internals()} == {3, 4}
    assert root not in tree.edges()
    assert tree.is_leaf(a)
    assert tree.is_internal(left)
    assert tree.parent_of(root) is None
    assert a in tree.children_of(left)
    assert tree.edge_length(a) == 1.0
    assert tree.edge_metadata(a) == "[&rate=0.5]"
    assert tree.nhx(a, "S") == "human"
    assert tree.nhx(a, "missing") is None
    assert tree.preorder()[0] == root
    assert tree.postorder()[-1] == root
    assert tree.to_newick() == str(tree)


def test_nonbinary_and_missing_lengths_do_not_seal():
    assert not TreeBuilder.from_newick("(A:1,B:2,C:3);").is_binary()
    with pytest.raises(RuntimeError, match="not binary"):
        TreeBuilder.from_newick("(A:1,B:2,C:3);").to_binary()
    with pytest.raises(RuntimeError, match="no edge length"):
        BinaryTree.from_newick("(A,B:2);")


@pytest.mark.parametrize(
    "operation",
    [
        lambda tree, node: tree.is_leaf(node),
        lambda tree, node: tree.children_of(node),
        lambda tree, node: tree.parent_of(node),
        lambda tree, node: tree.name(node),
        lambda tree, node: tree.node_metadata(node),
        lambda tree, node: tree.edge_length(node),
        lambda tree, node: tree.edge_metadata(node),
        lambda tree, node: tree.add_node(node),
        lambda tree, node: tree.add_edge(tree.root, node),
        lambda tree, node: tree.remove_edge(tree.root, node),
        lambda tree, node: tree.replace_parent(node, tree.root),
        lambda tree, node: tree.set_root(node),
        lambda tree, node: tree.set_name(node, "x"),
        lambda tree, node: tree.set_node_metadata(node, "x"),
        lambda tree, node: tree.set_edge_length(node, 1.0),
        lambda tree, node: tree.set_edge_metadata(node, "x"),
        lambda tree, node: tree.add_hybrid_edge(tree.root, node),
        lambda tree, node: tree.remove_hybrid_edge(tree.root, node),
    ],
)
def test_tree_rejects_invalid_node_ids(operation):
    tree = TreeBuilder.from_newick("(A:1,B:2);")
    invalid = max(
        TreeBuilder.from_newick("((A:1,B:2):3,C:4);").nodes(),
        key=lambda node: node.index,
    )
    with pytest.raises(RuntimeError, match="out of range"):
        operation(tree, invalid)


@pytest.mark.parametrize(
    "operation",
    [
        lambda tree, node: tree.is_leaf(node),
        lambda tree, node: tree.is_internal(node),
        lambda tree, node: tree.children_of(node),
        lambda tree, node: tree.parent_of(node),
        lambda tree, node: tree.name(node),
        lambda tree, node: tree.node_metadata(node),
        lambda tree, node: tree.edge_length(node),
        lambda tree, node: tree.edge_metadata(node),
        lambda tree, node: tree.nhx(node, "S"),
    ],
)
def test_binary_tree_rejects_invalid_node_ids(operation):
    tree = BinaryTree.from_newick("(A:1,B:2);")
    invalid = BinaryTree.from_newick("((A:1,B:2):3,C:4);").root
    with pytest.raises(RuntimeError, match="out of range"):
        operation(tree, invalid)


def test_tree_rejects_integer_node_ids():
    tree = TreeBuilder.from_newick("(A:1,B:2);")
    with pytest.raises(RuntimeError, match="Expected Node, Leaf, or Internal"):
        tree.is_leaf(cast(Any, 0))


def test_binary_children_reject_leaf():
    tree = BinaryTree.from_newick("(A:1,B:2);")
    with pytest.raises(RuntimeError, match="is a leaf"):
        tree.children_of(tree.leaves()[0])
