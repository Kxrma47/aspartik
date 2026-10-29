import argparse
from dataclasses import dataclass
from pathlib import Path
from time import perf_counter
from typing import Iterator, Literal

from aspartik.data import TaxonSet
from aspartik.data.tree import (
    BinaryTree,
    branch_score_matrix,
    robinson_foulds_matrix,
    triplet_distance_matrix,
)
from aspartik.rng import RNG

type Metric = Literal["robinson-foulds", "branch-score", "triplet"]

METRICS: tuple[Metric, ...] = ("robinson-foulds", "branch-score", "triplet")
LEAF_COUNTS: dict[Metric, tuple[int, ...]] = {
    "robinson-foulds": (100, 200, 500, 1_000),
    "branch-score": (10, 20, 50, 100),
    "triplet": (10, 20, 50, 100),
}


@dataclass(frozen=True)
class BenchmarkResult:
    metric: Metric
    tree_count: int
    leaf_count: int
    generation_seconds: float
    distance_seconds: float


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


def distance_matrix(metric, trees):
    match metric:
        case "robinson-foulds":
            return robinson_foulds_matrix(trees)
        case "branch-score":
            return branch_score_matrix(trees)
        case "triplet":
            return triplet_distance_matrix(trees)
        case _:
            raise ValueError(f"unknown distance metric: {metric}")


def random_trees(tree_count: int, leaf_count: int, seed: int) -> list[BinaryTree]:
    rng = RNG(seed)
    if tree_count < 1:
        raise ValueError("expected at least one tree")
    if leaf_count < 2:
        raise ValueError("expected at least two leaves")
    taxa = TaxonSet.ranged_ints(leaf_count)
    return [BinaryTree.random(taxa, rng) for _ in range(tree_count)]


def read_trees(path: Path, tree_count: int) -> list[BinaryTree]:
    trees = []
    taxa = None
    with path.open(encoding="utf-8") as source:
        for line in source:
            if len(trees) == tree_count:
                break
            newick = line.strip()
            if not newick:
                raise ValueError("expected one nonempty Newick tree per line")
            tree = BinaryTree.from_newick(newick, taxa)
            if taxa is None:
                taxa = tree.taxa
            trees.append(tree)
    if len(trees) != tree_count:
        raise ValueError(f"expected {tree_count} trees, found {len(trees)}")
    return trees


def run_benchmark(
    tree_count: int,
    leaf_count: int | None,
    seed: int,
    metric: Metric,
    trees_file: Path | None = None,
) -> BenchmarkResult:
    if tree_count < 1:
        raise ValueError("expected at least one tree")
    generation_start = perf_counter()
    if trees_file is None:
        if leaf_count is None:
            raise ValueError("expected a leaf count for random tree generation")
        trees = random_trees(tree_count, leaf_count, seed)
    else:
        trees = read_trees(trees_file, tree_count)
    generation_end = perf_counter()
    leaf_count = trees[0].num_leaves
    _ = distance_matrix(metric, trees[:2])
    start = perf_counter()
    _ = distance_matrix(metric, trees)
    end = perf_counter()

    return BenchmarkResult(
        metric, tree_count, leaf_count, generation_end - generation_start, end - start
    )


def run_benchmarks(
    tree_count: int = 10_000,
    seed: int = 4,
    metric: Metric | None = None,
    leaf_count: int | None = None,
    trees_file: Path | None = None,
) -> Iterator[BenchmarkResult]:
    for current_metric in (metric,) if metric is not None else METRICS:
        if trees_file is not None:
            sizes = (None,)
        elif leaf_count is not None:
            sizes = (leaf_count,)
        else:
            sizes = LEAF_COUNTS[current_metric]
        for size in sizes:
            yield run_benchmark(tree_count, size, seed, current_metric, trees_file)


def parse_cli_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("n", type=positive_int, nargs="?", default=10_000)
    parser.add_argument("--metric", choices=METRICS)
    parser.add_argument("--num-leaves", type=num_leaves)
    parser.add_argument("--seed", type=int, default=4)
    parser.add_argument("--trees-file", type=Path)
    args = parser.parse_args()
    if args.trees_file is not None and args.num_leaves is not None:
        parser.error("--num-leaves cannot be used with --trees-file")
    return args


def main():
    args = parse_cli_args()
    print("metric,trees,leaves,nodes,generation_seconds,distance_seconds", flush=True)
    for result in run_benchmarks(
        args.n, args.seed, args.metric, args.num_leaves, args.trees_file
    ):
        print(
            f"{result.metric},{result.tree_count},{result.leaf_count},"
            f"{result.leaf_count * 2 - 1},{result.generation_seconds:.6f},"
            f"{result.distance_seconds:.6f}",
            flush=True,
        )


if __name__ == "__main__":
    main()
