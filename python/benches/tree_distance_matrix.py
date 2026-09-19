import argparse
from time import perf_counter
from typing import Literal

from aspartik.data.tree import (
    BinaryTree,
    Tree,
    robinson_foulds_matrix,
    triplet_distance_matrix,
)
from aspartik.rng import RNG

Metric = Literal["rf", "branch-score", "triplet"]
METRICS = ("rf", "branch-score", "triplet")


def positive_int(value):
    value = int(value)
    if value < 1:
        raise argparse.ArgumentTypeError("expected a positive integer")
    return value


def num_leaves(value):
    value = int(value)
    if value < 2:
        raise argparse.ArgumentTypeError("expected at least two leaves")
    return value


def pairwise_matrix(trees, distance):
    matrix = [[0 for _ in trees] for _ in trees]
    for first in range(len(trees)):
        for second in range(first):
            value = distance(trees[first], trees[second])
            matrix[first][second] = value
            matrix[second][first] = value
    return matrix


def distance_matrix(metric, trees):
    match metric:
        case "rf":
            return robinson_foulds_matrix(trees)
        case "branch-score":
            return pairwise_matrix(trees, BinaryTree.branch_score)
        case "triplet":
            return triplet_distance_matrix(trees)
        case _:
            raise ValueError(f"unknown distance metric: {metric}")


def random_tree(metric, leaf_count, rng):
    tree = BinaryTree.random(leaf_count, rng)
    if metric != "branch-score":
        return tree

    # TODO: branch lengths in `BinaryTree.random`
    builder = Tree.from_newick(tree.to_newick())
    for node in builder.nodes():
        if node != builder.root:
            builder.set_edge_length(node, rng.random_float(0.1, 1.0))
    return builder.to_binary()


def random_trees(
    metric: Metric, tree_count: int, leaf_count: int, seed: int
) -> list[BinaryTree]:
    rng = RNG(seed)
    if tree_count < 1:
        raise ValueError("expected at least one tree")
    if leaf_count < 2:
        raise ValueError("expected at least two leaves")
    return [random_tree(metric, leaf_count, rng) for _ in range(tree_count)]


def run_benchmark(
    tree_count, leaf_count: int, seed: int, metric: Metric = "rf"
) -> float:
    trees = random_trees(metric, tree_count, leaf_count, seed)
    start = perf_counter()
    _ = distance_matrix(metric, trees)
    end = perf_counter()

    return end - start


def format_number(value):
    if isinstance(value, int):
        return str(value)
    return f"{value:.6g}"


def parse_cli_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("n", type=positive_int)
    parser.add_argument("--metric", choices=METRICS, default="rf")
    parser.add_argument("--num-leaves", type=num_leaves, default=100)
    parser.add_argument("--seed", type=int, default=4)
    return parser.parse_args()


def main():
    args = parse_cli_args()
    result = run_benchmark(
        args.n, leaf_count=args.num_leaves, seed=args.seed, metric=args.metric
    )
    print(f"{result}sec")


if __name__ == "__main__":
    main()
