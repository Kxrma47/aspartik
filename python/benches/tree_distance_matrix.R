args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 3L) {
  stop("expected metric, tree count, and Newick file", call. = FALSE)
}

metric <- args[[1L]]
tree_count <- suppressWarnings(as.integer(args[[2L]]))
if (is.na(tree_count) || tree_count < 1L) {
  stop("expected a positive tree count", call. = FALSE)
}
if (!(metric %in% c("robinson-foulds", "branch-score", "triplet"))) {
  stop("unknown distance metric", call. = FALSE)
}

package <- if (metric == "triplet") "Quartet" else "phangorn"
if (!suppressPackageStartupMessages(requireNamespace(package, quietly = TRUE))) {
  stop(sprintf("install the R package '%s' to benchmark %s", package, metric),
       call. = FALSE)
}

lines <- readLines(args[[3L]], n = tree_count, warn = FALSE)
if (length(lines) != tree_count || any(!nzchar(trimws(lines)))) {
  stop(sprintf("expected %d nonempty Newick lines", tree_count), call. = FALSE)
}
trees <- ape::read.tree(text = lines)
if (inherits(trees, "phylo")) {
  trees <- structure(list(trees), class = "multiPhylo")
}

distance_matrix <- function(trees) {
  if (length(trees) == 1L) return(matrix(0, 1L, 1L))
  result <- switch(metric,
                   "robinson-foulds" = phangorn::RF.dist(
                     trees, rooted = TRUE, check.labels = TRUE
                   ),
                   "branch-score" = phangorn::KF.dist(
                     trees, rooted = TRUE, check.labels = TRUE
                   ),
                   "triplet" = Quartet::TripletDistance(trees))
  as.matrix(result)
}

warmup <- distance_matrix(trees[seq_len(min(2L, tree_count))])
start <- proc.time()[["elapsed"]]
result <- distance_matrix(trees)
elapsed <- proc.time()[["elapsed"]] - start
stopifnot(nrow(result) == tree_count, ncol(result) == tree_count)
cat(sprintf("%.9f\n", elapsed))
