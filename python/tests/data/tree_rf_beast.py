import pytest

from pathlib import Path

from aspartik.data.tree import TreeBuilder, robinson_foulds_matrix

TREE_PATHS = sorted(Path("data/runs").glob("*/beast*.trees"))
assert TREE_PATHS


@pytest.mark.parametrize("path", TREE_PATHS, ids=lambda path: path.parent.name)
def test_robinson_foulds_matrix_on_beast_trees(path):
    builder = TreeBuilder()
    newicks = path.read_text().splitlines()
    builder.load_newick(newicks[0])
    first = builder.to_binary()
    taxa = first.taxa

    trees = [first]
    for newick in newicks[1:]:
        builder.load_newick(newick)
        trees.append(builder.to_binary(taxa))

    matrix = robinson_foulds_matrix(trees)

    assert trees
    assert len(matrix) == len(trees) ** 2
    assert all(matrix[index * len(trees) + index] == 0 for index in range(len(trees)))
