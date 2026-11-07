"""Install one built wheel into a fresh venv and run the Python suite against it."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test(directory: Path) -> None:
    wheels = list(directory.resolve().glob("*.whl"))
    assert len(wheels) == 1, "Provide a directory containing exactly one platform wheel"
    with tempfile.TemporaryDirectory(prefix="cfop-wheel-") as temporary:
        environment = Path(temporary) / "venv"
        subprocess.run(["uv", "venv", str(environment), "--python", sys.executable], check=True)
        python = environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        subprocess.run(
            ["uv", "pip", "install", "--python", str(python), str(wheels[0]), "pytest>=8.3,<10"],
            check=True,
        )
        # An unrelated working directory and isolated mode prevent the checkout
        # or an editable installation from masking a broken wheel.
        subprocess.run(
            [str(python), "-I", "-m", "pytest", "-q", str(ROOT / "tests/python_smoke.py")],
            cwd=temporary,
            check=True,
        )
        subprocess.run(
            [
                str(python),
                "-I",
                "-c",
                "import cfop_solver; print('Installed wheel:', cfop_solver.__file__)",
            ],
            cwd=temporary,
            check=True,
        )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    test(parser.parse_args().directory)
