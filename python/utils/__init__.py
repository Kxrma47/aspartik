from collections.abc import Callable, Sequence

from aspartik.b3 import Operator, Proposal
from aspartik.b3.parameters import DatedTree
from aspartik.data import TaxonSet
from aspartik.data.msa import MSA
from aspartik.rng import RNG


def all_pairs(parameters: Sequence[Sequence]) -> list[tuple]:
    """All pairs testing which mixes inputs without a combinatorial explosion

    See [the NIST article][n].

    [n]: https://csrc.nist.gov/Projects/Automated-Combinatorial-Testing-for-Software
    """

    if not parameters:
        return []
    if len(parameters) == 1:
        return [(parameters[0][0],)]

    cases = [[a, b] for a in parameters[0] for b in parameters[1]]
    seen = {(0, a, 1, b) for a in parameters[0] for b in parameters[1]}

    for col in range(2, len(parameters)):
        values = parameters[col]

        for case in cases:
            case.append(values[0])

        for val in values:
            while True:
                new_case = []
                covered = 0
                for earlier in range(col):

                    def score(cand):
                        return sum(
                            (earlier, cand, col, val) not in seen
                            for old in parameters[earlier]
                        )

                    best = max(parameters[earlier], key=score)
                    covered += score(best)
                    new_case.append(best)

                if covered == 0:
                    break
                new_case.append(val)
                seen.update((other, new_case[other], col, val) for other in range(col))
                cases.append(new_case)

    return [tuple(case) for case in cases]


def random_float(lower: float, upper: float, num: int = 100) -> list[float]:
    rng = RNG(4)
    return [rng.random_float(lower, upper) for _ in range(num)]


def random_integer(lower: int, upper: int, num: int = 100) -> list[int]:
    rng = RNG(4)
    return [rng.random_int(lower, upper) for _ in range(num)]


def check_tree_operator(factory: Callable[[DatedTree], Operator]) -> None:
    rng = RNG(4)
    tree = DatedTree(TaxonSet.ranged_ints(100), rng)

    operator = factory(tree)

    for _ in range(1000):
        proposal = operator.propose()
        if proposal == Proposal.Abort():
            tree.reject()
        else:
            tree.accept()

        tree.validate()


def random_tree(rng, lower: int, upper: int):
    len = rng.random_int(lower, upper)
    return DatedTree(TaxonSet.ranged_ints(len), rng)


def random_trees(lower: int, upper: int, num: int = 1000):
    rng = RNG(4)

    return [random_tree(rng, lower, upper) for _ in range(num)]


def random_msa(rng: RNG, lower: int, upper: int):
    num_sites = rng.random_int(lower, upper)
    num_sequences = rng.random_int(lower, upper)
    names = TaxonSet.ranged_ints(num_sequences)

    return MSA.random(num_sequences, num_sites, names, rng)


def random_msas(lower: int, upper: int, num: int):
    rng = RNG(4)

    return [random_msa(rng, lower, upper) for _ in range(num)]
