import pytest

from aspartik.data.tree import BinaryTree, Internal, Leaf, Node, TreeBuilder


def test_empty_tree():
    tree = TreeBuilder()
    with pytest.raises(RuntimeError, match="no root"):
        str(tree)

    root = tree.add_dummy_node()
    tree.set_root(root)
    assert str(tree) == ";"


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
    assert tree.preorder()[0] == root
    assert tree.postorder()[-1] == root
    assert tree.to_newick() == str(tree)


def test_binary_children_reject_leaf():
    tree = BinaryTree.from_newick("(A:1,B:2);")
    with pytest.raises(RuntimeError, match="is a leaf"):
        tree.children_of(tree.leaves()[0])
