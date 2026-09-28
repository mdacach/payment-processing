#!/usr/bin/env python3
"""Run every sample CSV and save its account output beside the input."""

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
SAMPLES = ROOT / "samples"
BINARY = ROOT / "target" / "debug" / "payment-processing"


def main() -> None:
    subprocess.run(["cargo", "build", "--quiet"], cwd=ROOT, check=True)

    inputs = sorted(SAMPLES.rglob("*.csv"))
    for input_path in inputs:
        result = subprocess.run(
            [str(BINARY), str(input_path.relative_to(ROOT))],
            cwd=ROOT,
            capture_output=True,
            check=False,
        )
        output_path = input_path.with_suffix(".output")
        if result.returncode == 0:
            output_path.write_bytes(result.stdout)
        else:
            if result.stdout:
                raise RuntimeError(f"{input_path}: failed after writing partial stdout")
            diagnostic = result.stderr.decode("utf-8", errors="replace").rstrip()
            output_path.write_text(
                "No account CSV output was produced.\n"
                f"Exit status: {result.returncode}\n\n"
                f"Standard error:\n{diagnostic}\n",
                encoding="utf-8",
            )

    print(f"Updated {len(inputs)} sample output files")


if __name__ == "__main__":
    main()
