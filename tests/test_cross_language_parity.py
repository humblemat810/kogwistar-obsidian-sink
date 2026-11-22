from __future__ import annotations

import shutil
import subprocess
import tempfile
from pathlib import Path

import pytest

from kogwistar_obsidian_sink.integrations.kogwistar_adapter import KogwistarDuckProvider
from kogwistar_obsidian_sink.sinks.obsidian import ObsidianVaultSink


def _run_rust(*arguments: str) -> str:
    if shutil.which("cargo") is None:
        pytest.skip("cargo is unavailable")
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            "rust/Cargo.toml",
            "--bin",
            "kogwistar-obsidian-sink",
            "--",
            *arguments,
        ],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        encoding="utf-8",
    )
    return result.stdout


def _relative_files(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in root.rglob("*")
        if path.is_file()
    }


def test_python_and_rust_build_from_export_match() -> None:
    with tempfile.TemporaryDirectory(prefix=".tmp_sink_parity_", dir=Path.cwd()) as work:
        work_path = Path(work)
        export_path = Path.cwd() / "examples" / "sample_graph_export.json"
        python_vault = work_path / "python-vault"
        rust_vault = work_path / "rust-vault"

        ObsidianVaultSink(python_vault).build(KogwistarDuckProvider.from_export_file(export_path))
        _run_rust(
            "build-from-export",
            "--input",
            str(export_path),
            "--vault",
            str(rust_vault),
        )

        assert _relative_files(python_vault) == _relative_files(rust_vault)
