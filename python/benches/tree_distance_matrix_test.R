source("python/benches/tree_distance_matrix.R")

path <- "python/benches/fixtures/tree_distance_matrix.newick"
trees <- read_trees(path, 3L)
rf <- distance_matrix("robinson-foulds", trees)
branch_score <- distance_matrix("branch-score", trees)
triplet <- distance_matrix("triplet", trees)

stopifnot(
  identical(dim(rf), c(3L, 3L)),
  rf[1L, 2L] == 4,
  rf[1L, 3L] == 0,
  branch_score[1L, 2L] == 4,
  branch_score[1L, 3L] == 1,
  triplet[1L, 2L] == 4,
  triplet[1L, 3L] == 0,
  all(diag(rf) == 0),
  all(diag(branch_score) == 0),
  all(diag(triplet) == 0)
)

short_input <- try(read_trees(path, 4L), silent = TRUE)
stopifnot(inherits(short_input, "try-error"))

stopifnot(all(distance_matrix("robinson-foulds", trees[1L]) == 0))
stopifnot(all(distance_matrix("branch-score", trees[1L]) == 0))
stopifnot(all(distance_matrix("triplet", trees[1L]) == 0))
