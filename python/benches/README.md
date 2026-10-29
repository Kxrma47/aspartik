# Tree distance matrix benchmarks

The Python and R scripts print the same CSV columns. The R script uses
`phangorn` for rooted Robinson-Foulds and Kuhner-Felsenstein branch score,
and `Quartet` for rooted triplet distance.

Install the R packages with
`Rscript -e 'install.packages(c("ape", "phangorn", "Quartet"))'`.

For a direct comparison, use a file containing one Newick tree per line.
Both scripts read the first `n` trees and time the complete matrix operation
after loading and warming up the distance function:

```sh
Rscript python/benches/tree_distance_matrix.R 100 --metric robinson-foulds --trees-file trees.newick
PYTHONPATH=python .venv/bin/python python/benches/tree_distance_matrix.py 100 --metric robinson-foulds --trees-file trees.newick
```

The same commands accept `branch-score` or `triplet` as the metric. All trees
must be rooted, binary, have the same named leaves, and include branch lengths
for branch score. `generation_seconds` measures input preparation (random
generation or Newick parsing); compare `distance_seconds` for matrix runtime.
The resulting matrices are dense, so memory rises quadratically with `n`.

Without `--trees-file`, each script generates random trees. Those runs use
different random tree generators and are not a direct speed comparison.
Run `Rscript python/benches/tree_distance_matrix_test.R` to check the R
results against the shared fixture.
