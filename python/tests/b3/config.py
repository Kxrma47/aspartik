import pytest
from utils import all_pairs

from aspartik.b3.config import MCMCConfig
from aspartik.b3.optimizers import BeastOptimizer, LinearOptimizer
from aspartik.data.msa import MSA

APES_MSA = MSA.from_fasta_file("data/alignments/apes.fasta")


parameters = [
    ["cpu"],  # calculator
    ["JC", "K80", "HKY", "GTR"],  # substitution_model
    ["classic", "default"],  # operator_mix
    [None, LinearOptimizer(1_000_000), BeastOptimizer()],  # optimizer
    [None, 1.0],  # clock_rate
    [
        "yule",
        "constant",
        "exponential",
        "bdsky",
        "bdsky:20",
        "skyline",
        "skyline:5",
    ],  # tree_prior
    [None, 100],  # print_every
    [None, "target/test_config.trace"],  # trace_path
    [None, "target/test_config.opstats"],  # opstats_path
    [False, True],  # timer
]


@pytest.mark.parametrize(
    "calculator, substitution_model, operator_mix, optimizer, clock_rate,"
    " tree_prior, print_every, trace_path, opstats_path, timer",
    all_pairs(parameters),
)
def test_config(
    calculator,
    substitution_model,
    operator_mix,
    optimizer,
    clock_rate,
    tree_prior,
    print_every,
    trace_path,
    opstats_path,
    timer,
):
    mcmc = MCMCConfig(
        APES_MSA,
        calculator=calculator,
        substitution_model=substitution_model,
        operator_mix=operator_mix,
        optimizer=optimizer,
        tree_prior=tree_prior,
        print_every=print_every,
        trace_path=trace_path,
        opstats_path=opstats_path,
        timer=timer,
    ).b3_mcmc()

    mcmc.run(10)
