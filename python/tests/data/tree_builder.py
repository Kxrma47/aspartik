import pytest

from aspartik.data.tree import BinaryTree, Tree


def test_empty_tree():
    assert str(Tree()) == ";"


def test_from_newick():
    assert str(Tree.from_newick("(A:1,B:2)root;")) == "(A:1,B:2)root;"


def node_named(tree, name):
    return next(node for node in tree.nodes() if tree.name(node) == name)


def test_tree_access_and_traversal():
    tree = Tree.from_newick("((A:1,B:2)left:3,C:4)root;")
    root = tree.root
    left = node_named(tree, "left")
    a = node_named(tree, "A")

    assert len(tree) == tree.num_nodes == 5
    assert tree.nodes() == list(range(5))
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
    tree = Tree()
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
    tree = Tree.from_newick("((A:1,B:2)left:3,(C:4,D:5)right:6)root;")
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
    tree = Tree.from_newick("((A:1,(C:1)X#H1:2)L:1,(X#H1:3,B:1)R:1)root;")
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
    assert len(tree) == tree.num_nodes == 5
    assert tree.num_leaves == 3
    assert tree.num_internals == 2
    assert tree.num_edges == 4
    assert tree.nodes() == list(range(5))
    assert tree.leaves() == [0, 1, 2]
    assert set(tree.internals()) == {3, 4}
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
    assert not Tree.from_newick("(A:1,B:2,C:3);").is_binary()
    with pytest.raises(RuntimeError, match="not binary"):
        Tree.from_newick("(A:1,B:2,C:3);").to_binary()
    with pytest.raises(RuntimeError, match="no edge length"):
        BinaryTree.from_newick("(A,B:2);")


@pytest.mark.parametrize(
    "operation",
    [
        lambda tree: tree.is_leaf(99),
        lambda tree: tree.children_of(99),
        lambda tree: tree.parent_of(99),
        lambda tree: tree.name(99),
        lambda tree: tree.node_metadata(99),
        lambda tree: tree.edge_length(99),
        lambda tree: tree.edge_metadata(99),
        lambda tree: tree.add_node(99),
        lambda tree: tree.add_edge(0, 99),
        lambda tree: tree.remove_edge(0, 99),
        lambda tree: tree.replace_parent(99, 0),
        lambda tree: tree.set_root(99),
        lambda tree: tree.set_name(99, "x"),
        lambda tree: tree.set_node_metadata(99, "x"),
        lambda tree: tree.set_edge_length(99, 1.0),
        lambda tree: tree.set_edge_metadata(99, "x"),
        lambda tree: tree.add_hybrid_edge(0, 99),
        lambda tree: tree.remove_hybrid_edge(0, 99),
    ],
)
def test_tree_rejects_invalid_node_ids(operation):
    with pytest.raises(RuntimeError, match="out of range"):
        operation(Tree.from_newick("(A:1,B:2);"))


@pytest.mark.parametrize(
    "operation",
    [
        lambda tree: tree.is_leaf(99),
        lambda tree: tree.is_internal(99),
        lambda tree: tree.children_of(99),
        lambda tree: tree.parent_of(99),
        lambda tree: tree.name(99),
        lambda tree: tree.node_metadata(99),
        lambda tree: tree.edge_length(99),
        lambda tree: tree.edge_metadata(99),
        lambda tree: tree.nhx(99, "S"),
    ],
)
def test_binary_tree_rejects_invalid_node_ids(operation):
    with pytest.raises(RuntimeError, match="out of range"):
        operation(BinaryTree.from_newick("(A:1,B:2);"))


def test_binary_children_reject_leaf():
    tree = BinaryTree.from_newick("(A:1,B:2);")
    with pytest.raises(RuntimeError, match="is a leaf"):
        tree.children_of(tree.leaves()[0])
