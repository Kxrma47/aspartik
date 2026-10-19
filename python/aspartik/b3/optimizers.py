from dataclasses import dataclass
from typing import Protocol


class Optimizer(Protocol):
    def tuning_rate(self, step: int) -> float: ...

    @property
    def every(self) -> int: ...


@dataclass(slots=True, frozen=True)
class LinearOptimizer(Optimizer):
    """
    Aggressive b3-specific optimizer

    This is the optimal optimizer for use with b3.  It sets a very high tuning
    rate until `cutoff`, quickly lowering it to 0 in the last tenth of the
    steps.  This enables operators to adapt to both the atypical region and
    then the stationary position.

    Note that because of the large tuning rate burn-in must be at least
    `cutoff` steps large.
    """

    cutoff: int
    magnitude: float = 0.5
    every: int = 10_000

    def tuning_rate(self, step: int) -> float:
        if step >= self.cutoff:
            return 0

        step_ratio = step / self.cutoff
        if step_ratio < 0.9:
            return self.magnitude
        else:
            return self.magnitude * 10 * (1.0 - step_ratio)


@dataclass(slots=True, frozen=True)
class BeastOptimizer(Optimizer):
    """
    An optimizer with a reciprocal tuning rate

    It follows the BEAST approach with a few important deviations:

    - Each adaptation is done `every` steps, so it doesn't depend on the number
      of proposals an operator has actually made.
    - Acceptance ratios are always smoothed, but they do not account for the
      Hastings ratio of each failure.
    - It does not support `log` and `sqrt` versions because those might not
      converge.
    """

    delay: int = 0
    """
    Disables tuning for the first `delay` steps.

    The skipped steps are subtracted, so the absolute values of the tuning rate
    will be the same when tuning starts, they are just shifted by `delay`
    steps.
    """

    every: int = 10_000

    def tuning_rate(self, step: int) -> float:
        if step < self.delay:
            return 0

        num_tunes = (step - self.delay) / self.every
        return 1 / (num_tunes + 2)
