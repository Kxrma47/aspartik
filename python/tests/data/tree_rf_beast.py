import pytest

from pathlib import Path

from aspartik.data.tree import BinaryTree, Tree, robinson_foulds_matrix

TREE_PATHS = sorted(Path("data/runs").glob("*/beast*.trees"))
assert TREE_PATHS


def canonical_tree(newick, leaf_names):
    source = Tree.from_newick(newick)
    source_leaves = {}
    for node in source.nodes():
        if source.is_leaf(node):
            name = source.name(node)
            assert name is not None
            source_leaves[name] = node
    assert set(source_leaves) == set(leaf_names)

    target = Tree()
    mapping = {source.root: target.root}
    for name in leaf_names:
        mapping[source_leaves[name]] = target.add_node(target.root, name, 0.0)
    for node in source.nodes():
        if node != source.root and not source.is_leaf(node):
            mapping[node] = target.add_node(target.root, length=0.0)
    for node in source.nodes():
        if node != source.root:
            parent = source.parent_of(node)
            assert parent is not None
            target.replace_parent(mapping[node], mapping[parent])
    return target.to_binary()


@pytest.mark.parametrize("path", TREE_PATHS, ids=lambda path: path.parent.name)
def test_robinson_foulds_matrix_on_beast_trees(path):
    newick = path.read_text().splitlines()
    first = BinaryTree.from_newick(newick[0])
    leaf_names = []
    for node in first.leaves():
        name = first.name(node)
        assert name is not None
        leaf_names.append(name)
    leaf_names.sort()
    trees = [canonical_tree(line, leaf_names) for line in newick]
    matrix = robinson_foulds_matrix(trees)

    assert trees
    assert len(matrix) == len(trees) ** 2
    assert all(matrix[index * len(trees) + index] == 0 for index in range(len(trees)))
