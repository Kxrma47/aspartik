import pytest
from benches.tree_distance_matrix import METRICS, run_benchmark

import subprocess
import sys
from pathlib import Path

BENCHMARK = Path(__file__).parents[2] / "benches" / "tree_distance_matrix.py"


def run_cli(*arguments):
    return subprocess.run(
        [sys.executable, str(BENCHMARK), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )


@pytest.mark.parametrize("metric", METRICS)
def test_tree_distance_matrix_benchmark(metric):
    result = run_cli(
        "8",
        "--metric",
        metric,
        "--num-leaves",
        "16",
        "--repeats",
        "2",
    )
    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert lines[0] == f"metric={metric} trees=8 leaves=16 seed=4 repeats=2"
    assert lines[1] == "repeat\tgeneration_s\tdistance_matrix_s\tmaximum\tchecksum"
    assert lines[2].startswith("1\t")
    assert lines[3].startswith("2\t")
    assert lines[4].startswith("median\t")


@pytest.mark.parametrize("metric", METRICS)
def test_runner_returns_results_without_printing(metric, capsys):
    result = run_benchmark(6, leaf_count=12, seed=5, repeats=2, metric=metric)

    assert capsys.readouterr().out == ""
    assert result.metric == metric
    assert result.tree_count == 6
    assert result.leaf_count == 12
    assert result.seed == 5
    assert len(result.runs) == 2
    assert result.generation_median >= 0
    assert result.matrix_median >= 0
    assert result.runs[0].maximum == result.runs[1].maximum
    assert result.runs[0].checksum == result.runs[1].checksum


def test_branch_score_uses_nonzero_edge_lengths():
    result = run_benchmark(
        6,
        leaf_count=12,
        seed=5,
        repeats=1,
        metric="branch-score",
    )

    assert result.runs[0].maximum > 0
    assert result.runs[0].checksum > 0


def test_benchmark_rejects_invalid_arguments():
    for arguments in [
        ("0",),
        ("1", "--num-leaves", "1"),
        ("1", "--repeats", "0"),
        ("1", "--metric", "unknown"),
    ]:
        result = run_cli(*arguments)
        assert result.returncode == 2
        assert "error:" in result.stderr


@pytest.mark.parametrize(
    ("arguments", "message"),
    [
        ({"tree_count": 0}, "at least one tree"),
        ({"tree_count": 1, "leaf_count": 1}, "at least two leaves"),
        ({"tree_count": 1, "repeats": 0}, "at least one repeat"),
        ({"tree_count": 1, "metric": "unknown"}, "unknown distance metric"),
    ],
)
def test_runner_rejects_invalid_arguments(arguments, message):
    with pytest.raises(ValueError, match=message):
        run_benchmark(**arguments)
