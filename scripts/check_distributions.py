"""Validate versions, metadata, notices, typing files, and distribution hygiene."""

from __future__ import annotations

import argparse
import email
import re
import tarfile
import zipfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
NOTICES = ("LICENSE", "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_DEPENDENCIES.txt")


def check_paths(names: list[str]) -> None:
    for name in names:
        path = PurePosixPath(name)
        assert not path.is_absolute() and ".." not in path.parts, name
        assert not set(path.parts) & {".venv", "target", ".git", "__pycache__"}, name
        assert not path.name.startswith(".env"), name
        assert not name.endswith((".jsonl", ".pyc")), name


def check(directory: Path) -> None:
    versions = [
        re.search(r'^version = "([^"]+)"', (ROOT / name).read_text(), re.MULTILINE)
        for name in ("Cargo.toml", "pyproject.toml")
    ]
    assert all(versions), "Missing project version"
    version = versions[0].group(1)  # type: ignore[union-attr]
    assert versions[1].group(1) == version, "Rust/Python versions differ"  # type: ignore[union-attr]
    wheels = sorted(directory.glob("*.whl"))
    sources = sorted(directory.glob("*.tar.gz"))
    assert wheels and sources, "Build both a wheel and an sdist before checking"
    for wheel in wheels:
        with zipfile.ZipFile(wheel) as archive:
            names = archive.namelist()
            check_paths(names)
            metadata = email.message_from_bytes(
                archive.read(next(name for name in names if name.endswith(".dist-info/METADATA")))
            )
            assert metadata["Version"] == version, wheel
            assert metadata["Requires-Python"] == ">=3.9", wheel
            assert metadata["License-Expression"] == "MIT", wheel
            assert metadata["Project-URL"], "Missing project links"
            assert "cfop_solver/py.typed" in names
            assert "cfop_solver/__init__.pyi" in names
            assert any(name.endswith((".so", ".pyd")) for name in names)
            for notice in NOTICES:
                expected = (ROOT / notice).read_bytes()
                members = [name for name in names if name.endswith("/licenses/" + notice)]
                assert len(members) == 1 and archive.read(members[0]) == expected, notice
    for source in sources:
        with tarfile.open(source) as archive:
            names = archive.getnames()
            check_paths(names)
            for relative in (
                *NOTICES,
                "Cargo.lock",
                "uv.lock",
                "cfop_solver.pyi",
                "src/lib.rs",
                "tests/python_smoke.py",
            ):
                member = next(name for name in names if name.endswith("/" + relative))
                content = archive.extractfile(member)
                assert content is not None and content.read() == (ROOT / relative).read_bytes(), (
                    relative
                )
            assert archive.getnames()[0].split("/")[0].endswith(version), source
    print(f"Validated {len(wheels)} wheel(s) and {len(sources)} sdist(s) for {version}.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    check(parser.parse_args().directory)
