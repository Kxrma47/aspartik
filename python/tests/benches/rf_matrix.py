import subprocess
import sys
from pathlib import Path

BENCHMARK = Path(__file__).parents[2] / "benches" / "rf_matrix.py"


def run_benchmark(*arguments):
    return subprocess.run(
        [sys.executable, str(BENCHMARK), *arguments],
        check=False,
        capture_output=True,
        text=True,
    )


def test_rf_matrix_benchmark():
    result = run_benchmark("8", "--num-leaves", "16", "--repeats", "2")
    assert result.returncode == 0, result.stderr
    lines = result.stdout.splitlines()
    assert lines[0] == "trees=8 leaves=16 seed=4 repeats=2"
    assert lines[1] == "repeat\tgeneration_s\trf_matrix_s\tmax_rf\tchecksum"
    assert lines[2].startswith("1\t")
    assert lines[3].startswith("2\t")
    assert lines[4].startswith("median\t")


def test_rf_matrix_benchmark_rejects_invalid_sizes():
    for arguments in [
        ("0",),
        ("1", "--num-leaves", "1"),
        ("1", "--repeats", "0"),
    ]:
        result = run_benchmark(*arguments)
        assert result.returncode == 2
        assert "error:" in result.stderr
