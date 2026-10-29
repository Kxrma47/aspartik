metrics <- c("robinson-foulds", "branch-score", "triplet")
leaf_counts <- list(
  "robinson-foulds" = c(100L, 200L, 500L, 1000L),
  "branch-score" = c(10L, 20L, 50L, 100L),
  "triplet" = c(10L, 20L, 50L, 100L)
)

positive_integer <- function(value, minimum = 1L) {
  parsed <- suppressWarnings(as.integer(value))
  if (is.na(parsed) || parsed < minimum) {
    stop(sprintf("expected an integer >= %d", minimum), call. = FALSE)
  }
  parsed
}

parse_args <- function(args) {
  options <- list(n = 10000L, metric = NULL, num_leaves = NULL,
                  seed = 4L, trees_file = NULL)
  index <- 1L
  positional <- FALSE
  while (index <= length(args)) {
    argument <- args[[index]]
    if (startsWith(argument, "--")) {
      if (!(argument %in% c("--metric", "--num-leaves", "--seed", "--trees-file")) ||
          index == length(args)) {
        stop(sprintf("invalid option: %s", argument), call. = FALSE)
      }
      value <- args[[index + 1L]]
      key <- switch(argument,
                    "--metric" = "metric",
                    "--num-leaves" = "num_leaves",
                    "--seed" = "seed",
                    "--trees-file" = "trees_file")
      options[[key]] <- value
      index <- index + 2L
    } else {
      if (positional) stop("expected one tree count", call. = FALSE)
      options$n <- positive_integer(argument)
      positional <- TRUE
      index <- index + 1L
    }
  }
  if (!is.null(options$metric) && !(options$metric %in% metrics)) {
    stop("unknown distance metric", call. = FALSE)
  }
  if (!is.null(options$num_leaves)) {
    options$num_leaves <- positive_integer(options$num_leaves, 2L)
  }
  if (!is.null(options$trees_file) && !is.null(options$num_leaves)) {
    stop("--num-leaves cannot be used with --trees-file", call. = FALSE)
  }
  options$seed <- suppressWarnings(as.integer(options$seed))
  if (is.na(options$seed)) stop("expected an integer seed", call. = FALSE)
  options
}

require_metric_package <- function(metric) {
  package <- if (metric == "triplet") "Quartet" else "phangorn"
  if (!suppressPackageStartupMessages(requireNamespace(package, quietly = TRUE))) {
    stop(sprintf("install the R package '%s' to benchmark %s", package, metric),
         call. = FALSE)
  }
}

read_trees <- function(path, tree_count) {
  source <- if (grepl("\\.gz$", path)) gzfile(path, "rt") else file(path, "rt")
  on.exit(close(source))
  lines <- readLines(source, n = tree_count, warn = FALSE)
  if (length(lines) != tree_count || any(!nzchar(trimws(lines)))) {
    stop(sprintf("expected %d nonempty Newick lines", tree_count), call. = FALSE)
  }
  trees <- ape::read.tree(text = lines)
  if (inherits(trees, "phylo")) trees <- structure(list(trees), class = "multiPhylo")
  reference <- sort(trees[[1L]]$tip.label)
  if (any(vapply(trees, function(tree) !identical(sort(tree$tip.label), reference),
                 logical(1L)))) {
    stop("trees must share the same leaf names", call. = FALSE)
  }
  trees
}

random_trees <- function(tree_count, leaf_count, seed) {
  set.seed(seed)
  names <- as.character(seq_len(leaf_count) - 1L)
  trees <- replicate(tree_count, ape::rtree(leaf_count, tip.label = names),
                     simplify = FALSE)
  class(trees) <- "multiPhylo"
  trees
}

distance_matrix <- function(metric, trees) {
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

run_benchmark <- function(tree_count, leaf_count, seed, metric, trees_file = NULL) {
  require_metric_package(metric)
  generation_start <- proc.time()[["elapsed"]]
  trees <- if (is.null(trees_file)) {
    random_trees(tree_count, leaf_count, seed)
  } else {
    read_trees(trees_file, tree_count)
  }
  generation_end <- proc.time()[["elapsed"]]
  distance_matrix(metric, trees[seq_len(min(2L, tree_count))])
  start <- proc.time()[["elapsed"]]
  result <- distance_matrix(metric, trees)
  end <- proc.time()[["elapsed"]]
  stopifnot(nrow(result) == tree_count, ncol(result) == tree_count)
  list(metric = metric, tree_count = tree_count,
       leaf_count = length(trees[[1L]]$tip.label),
       generation_seconds = generation_end - generation_start,
       distance_seconds = end - start)
}

run_benchmarks <- function(options) {
  selected <- if (is.null(options$metric)) metrics else options$metric
  for (metric in selected) {
    sizes <- if (!is.null(options$trees_file)) {
      NA_integer_
    } else if (!is.null(options$num_leaves)) {
      options$num_leaves
    } else {
      leaf_counts[[metric]]
    }
    for (size in sizes) {
      result <- run_benchmark(options$n, size, options$seed, metric,
                              options$trees_file)
      cat(sprintf("%s,%d,%d,%d,%.6f,%.6f\n",
                  result$metric, result$tree_count, result$leaf_count,
                  2L * result$leaf_count - 1L, result$generation_seconds,
                  result$distance_seconds))
      flush(stdout())
    }
  }
}

if (sys.nframe() == 0L) {
  options <- parse_args(commandArgs(trailingOnly = TRUE))
  cat("metric,trees,leaves,nodes,generation_seconds,distance_seconds\n")
  run_benchmarks(options)
}
