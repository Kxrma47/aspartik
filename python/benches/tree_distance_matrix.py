"""Benchmark dense tree distance matrices with Aspartik, R, or both.

Run from the repository root with:

    PYTHONPATH=python .venv/bin/python python/benches/tree_distance_matrix.py 100 --metric robinson-foulds --num-leaves 20 --implementation both

Use --implementation aspartik or --implementation r to run just one engine.
The R engine requires Rscript and the ape, phangorn, and Quartet packages.
Install them with Rscript -e 'install.packages(c("ape", "phangorn", "Quartet"))'.

Python generates each tree set once. The R engine reads those same trees from
a temporary Newick file that is removed after the run. Both engines warm their
distance function before timing the complete matrix calculation. The
generation_seconds column measures Python tree generation, while
distance_seconds excludes R startup and Newick parsing. The matrices are
dense, so their memory use grows quadratically with the tree count.
"""

import argparse
import subprocess
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
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
type Implementation = Literal["aspartik", "r"]

METRICS: tuple[Metric, ...] = ("robinson-foulds", "branch-score", "triplet")
LEAF_COUNTS: dict[Metric, tuple[int, ...]] = {
    "robinson-foulds": (100, 200, 500, 1_000),
    "branch-score": (10, 20, 50, 100),
    "triplet": (10, 20, 50, 100),
}


@dataclass(frozen=True)
class BenchmarkResult:
    implementation: Implementation
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


def aspartik_distance_seconds(metric: Metric, trees: list[BinaryTree]) -> float:
    distance_matrix(metric, trees[:2])
    start = perf_counter()
    distance_matrix(metric, trees)
    return perf_counter() - start


def r_distance_seconds(metric: Metric, trees: list[BinaryTree]) -> float:
    with TemporaryDirectory() as directory:
        trees_file = Path(directory) / "trees.newick"
        with trees_file.open("w", encoding="utf-8") as output:
            for tree in trees:
                output.write(tree.to_newick() + "\n")
        result = subprocess.run(
            [
                "Rscript",
                str(Path(__file__).with_suffix(".R")),
                metric,
                str(len(trees)),
                str(trees_file),
            ],
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
    return float(result.stdout.strip())


def run_benchmarks(
    tree_count: int = 10_000,
    seed: int = 4,
    metric: Metric | None = None,
    leaf_count: int | None = None,
    implementation: Implementation | Literal["both"] = "both",
) -> Iterator[BenchmarkResult]:
    engines: tuple[Implementation, ...] = (
        ("aspartik", "r") if implementation == "both" else (implementation,)
    )
    for current_metric in (metric,) if metric is not None else METRICS:
        sizes = (leaf_count,) if leaf_count is not None else LEAF_COUNTS[current_metric]
        for size in sizes:
            start = perf_counter()
            trees = random_trees(tree_count, size, seed)
            generation_seconds = perf_counter() - start
            for engine in engines:
                distance_seconds = (
                    aspartik_distance_seconds(current_metric, trees)
                    if engine == "aspartik"
                    else r_distance_seconds(current_metric, trees)
                )
                yield BenchmarkResult(
                    engine,
                    current_metric,
                    tree_count,
                    size,
                    generation_seconds,
                    distance_seconds,
                )


def parse_cli_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("n", type=positive_int, nargs="?", default=10_000)
    parser.add_argument("--metric", choices=METRICS)
    parser.add_argument("--num-leaves", type=num_leaves)
    parser.add_argument("--seed", type=int, default=4)
    parser.add_argument(
        "--implementation", choices=("aspartik", "r", "both"), default="both"
    )
    return parser.parse_args()


def main():
    args = parse_cli_args()
    print(
        "implementation,metric,trees,leaves,nodes,generation_seconds,distance_seconds",
        flush=True,
    )
    for result in run_benchmarks(
        args.n, args.seed, args.metric, args.num_leaves, args.implementation
    ):
        print(
            f"{result.implementation},{result.metric},{result.tree_count},"
            f"{result.leaf_count},{result.leaf_count * 2 - 1},"
            f"{result.generation_seconds:.6f},"
            f"{result.distance_seconds:.6f}",
            flush=True,
        )


if __name__ == "__main__":
    main()
