import pytest

from aspartik.data.tree import BinaryTree


def test_branch_score():
    first = BinaryTree.from_newick("(A:1,B:2);")
    second = BinaryTree.from_newick("(A:2,B:4);")

    assert first.branch_score(first) == 0.0
    assert first.branch_score(second) == pytest.approx(5.0**0.5)
    assert second.branch_score(first) == pytest.approx(5.0**0.5)
