import pytest

from aspartik.data.tree import BinaryTree
from aspartik.rng import RNG


def topology(tree):
    return [tree.children_of(node) for node in tree.internals()]


def test_random_tree():
    tree = BinaryTree.random(100, RNG(4))

    assert tree.num_leaves == 100
    assert tree.num_nodes == 199
    assert tree.num_edges == 198
    assert len(tree.preorder()) == tree.num_nodes
    assert tree.preorder()[0] == tree.root
    assert sorted(tree.postorder()) == tree.nodes()
    assert all(tree.name(node) is None for node in tree.nodes())
    assert all(tree.node_metadata(node) is None for node in tree.nodes())
    assert all(tree.edge_length(child) == 0.0 for child in tree.edges())
    assert all(tree.edge_metadata(child) is None for child in tree.edges())


def test_random_tree_is_deterministic():
    first = BinaryTree.random(100, RNG(4))
    second = BinaryTree.random(100, RNG(4))
    assert topology(first) == topology(second)


def test_random_tree_advances_rng():
    rng = RNG(4)
    first = BinaryTree.random(100, rng)
    second = BinaryTree.random(100, rng)
    assert topology(first) != topology(second)


@pytest.mark.parametrize("num_leaves", [0, 1, 2**32])
def test_random_tree_rejects_invalid_sizes(num_leaves):
    with pytest.raises((OverflowError, RuntimeError)):
        BinaryTree.random(num_leaves, RNG(4))


def test_branch_score():
    first = BinaryTree.from_newick("(A:1,B:2);")
    second = BinaryTree.from_newick("(A:2,B:4);")

    assert first.branch_score(first) == 0.0
    assert first.branch_score(second) == pytest.approx(5.0**0.5)
    assert second.branch_score(first) == pytest.approx(5.0**0.5)
