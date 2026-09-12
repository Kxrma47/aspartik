from .._aspartik_rust_impl._data_rust_impl import (
    BinaryTree as BinaryTree,
    Tree as Tree,
    TreeCollection as TreeCollection,
)


def robinson_foulds_matrix(trees: TreeCollection):
    import numpy as np

    return np.frombuffer(trees._robinson_foulds_matrix(), dtype=np.uint32).reshape(
        (len(trees), len(trees))
    )
