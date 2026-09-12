from aspartik.data.tree import Tree


def test_empty_tree():
    assert str(Tree()) == ";"


def test_from_newick():
    assert str(Tree.from_newick("(A:1,B:2)root;")) == "(A:1,B:2)root;"
