import pytest

from pathlib import Path

from aspartik.data import TaxonSet
from aspartik.data.tree import BinaryTree, robinson_foulds_matrix

TREE_PATHS = sorted(Path("data/runs").glob("*/beast*.trees"))
assert TREE_PATHS


@pytest.mark.parametrize("path", TREE_PATHS, ids=lambda path: path.parent.name)
def test_robinson_foulds_matrix_on_beast_trees(path):
    newick = path.read_text().splitlines()
    first = BinaryTree.from_newick(newick[0])
    leaf_names = []
    for node in first.leaves():
        name = first.name(node)
        assert name is not None
        leaf_names.append(name)
    taxa = TaxonSet(sorted(leaf_names))
    trees = [BinaryTree.from_newick(line, taxa) for line in newick]
    matrix = robinson_foulds_matrix(trees)

    assert trees
    assert len(matrix) == len(trees) ** 2
    assert all(matrix[index * len(trees) + index] == 0 for index in range(len(trees)))
