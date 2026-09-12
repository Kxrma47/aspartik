import pytest

from xml.etree import ElementTree

from aspartik.data.tree import BinaryTree, SvgOptions


def test_rectangular_and_tidy_coordinates():
    tree = BinaryTree.from_newick("((A:1,B:3):2,(C:2,D:4):1);")
    rectangular = tree.layout("rectangular", separation=2.0)
    tidy = tree.layout("tidy", separation=2.0)

    assert len(rectangular) == tree.num_nodes
    assert len(tidy) == tree.num_nodes
    assert rectangular[tree.root][0] == 0.0
    assert tidy[tree.root][0] == 0.0
    assert max(y for _, y in tidy) <= max(y for _, y in rectangular)
    for child in tree.edges():
        parent = tree.parent_of(child)
        assert parent is not None
        assert rectangular[child][0] >= rectangular[parent][0]
        assert tidy[child][0] >= tidy[parent][0]


@pytest.mark.parametrize("kind", ["rectangular", "tidy"])
def test_svg_is_valid_and_complete(kind):
    tree = BinaryTree.from_newick(
        "(('A<&':1[&rate='x&y'],B:2)'inner<&':3,C:4)'root<&';"
    )
    svg = tree.to_svg(
        kind,
        separation=1.5,
        options=SvgOptions(
            x_scale=80.0,
            y_scale=24.0,
            margin=12.0,
            node_radius=2.5,
            font_size=11.0,
        ),
        node_color="#336699",
        edge_color="#999999",
    )
    root = ElementTree.fromstring(svg)
    namespace = {"svg": "http://www.w3.org/2000/svg"}

    assert root.tag == "{http://www.w3.org/2000/svg}svg"
    assert len(root.findall("svg:circle", namespace)) == tree.num_nodes
    edge_tag = "svg:path" if kind == "rectangular" else "svg:line"
    assert len(root.findall(edge_tag, namespace)) == tree.num_edges
    assert len(root.findall("svg:text", namespace)) == tree.num_leaves
    assert "A<&" in "".join(root.itertext())
    assert {
        circle.attrib["fill"] for circle in root.findall("svg:circle", namespace)
    } == {"#336699"}


def test_rendering_rejects_invalid_options():
    tree = BinaryTree.from_newick("(A:1,B:2);")
    with pytest.raises(Exception, match="Unknown tree layout"):
        tree.layout("radial")
    with pytest.raises(Exception, match="separation"):
        tree.layout(separation=0.0)
    with pytest.raises(Exception, match="Horizontal scale"):
        tree.to_svg(options=SvgOptions(x_scale=0.0))
