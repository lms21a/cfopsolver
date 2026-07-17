from os import PathLike
from typing import Any

SOLVED_STATE: str
STATE_FORMAT: str
DECISION_TRACE_SCHEMA: str
CUBE_FEATURE_SCHEMA: str
CUBIE_FEATURE_FORMAT: str
FACELET_ID_FORMAT: str
CUBIE_FEATURE_WIDTH: int
CUBIE_ONE_HOT_WIDTH: int
FACELET_ID_WIDTH: int
FACELET_ONE_HOT_WIDTH: int
CUBIE_FEATURE_CARDINALITIES: list[int]
VERIFICATION_SCHEMA: str
__version__: str

class Cube:
    def __init__(self, state: str | None = None) -> None: ...
    @staticmethod
    def solved() -> Cube: ...
    @staticmethod
    def from_scramble(scramble: str) -> Cube: ...
    @property
    def state(self) -> str: ...
    @property
    def is_solved(self) -> bool: ...
    def apply(self, moves: str) -> None: ...
    def applied(self, moves: str) -> Cube: ...
    def verify(self, solution: str) -> bool: ...
    def serialize(self) -> str: ...
    def features(self, selection: str = "both") -> dict[str, Any] | None: ...
    def feature_vector(self, encoding: str = "cubie_ids") -> list[int]: ...

class Solver:
    def __init__(
        self,
        method: str = "cfop",
        trace: bool = False,
        exploration: bool = False,
        max_exploration_states: int = 72,
        alternatives: int = 0,
        max_depth: int = 30,
        probe_limit: int = 1_000_000,
        cross: str = "cross",
        f2l: str = "lookahead",
        efficiency: str = "balanced",
        decisions: bool = False,
        max_decision_records: int = 512,
    ) -> None: ...
    def solve(self, cube: Cube) -> dict[str, Any]: ...
    def solve_state(self, state: str) -> dict[str, Any]: ...
    def solve_scramble(self, scramble: str) -> dict[str, Any]: ...

class TrainingVerifier:
    def __init__(self, threads: int = 0, features: str = "none") -> None: ...
    def verify(self, item: dict[str, Any]) -> dict[str, Any]: ...
    def verify_batch(self, items: list[dict[str, Any]]) -> dict[str, Any]: ...

def apply_moves(state: str, moves: str) -> str: ...
def verify_solution(state: str, solution: str) -> bool: ...
def cube_features(state: str, selection: str = "both") -> dict[str, Any] | None: ...
def verify_batch(
    items: list[dict[str, Any]],
    threads: int = 0,
    features: str = "none",
) -> dict[str, Any]: ...
def generate(
    count: int,
    seed: int = 0,
    difficulty: str = "hard",
    scramble_length: int = 0,
    solutions_per_scramble: int = 1,
    method: str = "fast",
    threads: int = 0,
    trace: bool = False,
    exploration: bool = False,
    cross: str = "cross",
    f2l: str = "lookahead",
    efficiency: str = "balanced",
    max_depth: int = 30,
    probe_limit: int = 1_000_000,
    decisions: bool = False,
    max_decision_records: int = 512,
    features: str = "none",
) -> list[dict[str, Any]]: ...
def generate_jsonl(
    path: str | PathLike[str],
    count: int,
    seed: int = 0,
    difficulty: str = "hard",
    scramble_length: int = 0,
    solutions_per_scramble: int = 1,
    method: str = "fast",
    threads: int = 0,
    batch_size: int = 1024,
    trace: bool = False,
    exploration: bool = False,
    cross: str = "cross",
    f2l: str = "lookahead",
    efficiency: str = "balanced",
    max_depth: int = 30,
    probe_limit: int = 1_000_000,
    decisions: bool = False,
    max_decision_records: int = 512,
    features: str = "none",
) -> None: ...
