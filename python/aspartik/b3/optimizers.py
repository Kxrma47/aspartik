from dataclasses import dataclass
from typing import Protocol


class Optimizer(Protocol):
    def tuning_rate(self, step: int) -> float: ...

    @property
    def every(self) -> int: ...


@dataclass(slots=True)
class LinearOptimizer(Optimizer):
    cutoff: int
    every: int = 10_000

    def tuning_rate(self, step: int) -> float:
        if step >= self.cutoff:
            return 0

        step_ratio = step / self.cutoff
        return 0.5 if step_ratio < 0.9 else 5.0 * (1.0 - step_ratio)


@dataclass(slots=True)
class BeastOptimizer(Optimizer):
    delay: int = 0
    every: int = 10_000

    def tuning_rate(self, step: int) -> float:
        if step >= self.delay:
            return 0

        return 1 / (step / 100_000 + 2)
