import pytest
from benches.tree_distance_matrix import distance_matrix, read_trees, run_benchmark

from pathlib import Path

FIXTURE = Path(__file__).parents[2] / "benches/fixtures/tree_distance_matrix.newick"


def test_read_trees():
    trees = read_trees(FIXTURE, 3)
    assert len(trees) == 3
    assert all(tree.num_leaves == 4 for tree in trees)
    assert list(distance_matrix("robinson-foulds", trees)) == [
        0,
        4,
        0,
        4,
        0,
        4,
        0,
        4,
        0,
    ]
    branch_score = distance_matrix("branch-score", trees)
    assert branch_score[1] == 4
    assert branch_score[2] == 1
    assert list(distance_matrix("triplet", trees)) == [
        0,
        4,
        0,
        4,
        0,
        4,
        0,
        4,
        0,
    ]


def test_read_trees_requires_enough_input():
    with pytest.raises(ValueError, match="expected 4 trees, found 3"):
        read_trees(FIXTURE, 4)


def test_benchmark_shared_input():
    result = run_benchmark(3, None, 4, "robinson-foulds", FIXTURE)
    assert result.tree_count == 3
    assert result.leaf_count == 4
    assert result.generation_seconds >= 0
    assert result.distance_seconds >= 0
