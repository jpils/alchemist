import argparse
import gc
import os
from typing import Iterable

import numpy as np
import torch
from ase import Atoms
from ase.io import iread
from upet.calculator import UPETCalculator

DEFAULT_MODEL = "pet-mad-s"
DEFAULT_VERSION = "1.5.0"


def get_device(requested: str = "auto") -> str:
    if requested != "auto":
        if requested == "cpu":
            os.environ["WARP_DEVICE"] = "cpu"
        print(f"Using {requested.upper()}")
        return requested

    if torch.cuda.is_available():
        print("Using CUDA")
        return "cuda"

    os.environ["WARP_DEVICE"] = "cpu"
    print("Using CPU")
    return "cpu"


def build_calculator(model: str, version: str, device: str) -> UPETCalculator:
    return UPETCalculator(model=model, version=version, device=device)


def energy_uncertainty(calc: UPETCalculator, atoms: Atoms) -> float:
    value = calc.get_energy_uncertainty(atoms)
    return float(np.asarray(value).ravel()[0])


def energy_ensemble_rms(calc: UPETCalculator, atoms: Atoms) -> float:
    """RMS disagreement of UPET's built-in energy ensemble."""
    ensemble = np.asarray(calc.get_energy_ensemble(atoms), dtype=float).ravel()
    mean = ensemble.mean()
    return float(np.sqrt(((ensemble - mean) ** 2).mean()))


def force_committee_rms(calcs: Iterable[UPETCalculator], atoms: Atoms) -> tuple[float, float]:
    """Same force committee metric as rms.py, using several UPET calculators."""
    forces = []
    for model_idx, calc in enumerate(calcs):
        print(f"Calculating forces for model {model_idx}")
        atoms.calc = calc
        forces.append(atoms.get_forces())
        atoms.calc = None

    forces = np.asarray(forces, dtype=float)
    mean = forces.mean(axis=0)
    m = forces.shape[0]
    var_per_atom = ((forces - mean) ** 2).sum(axis=-1).mean(axis=0) / m

    rms = float(np.sqrt(var_per_atom.mean()))
    mean_norm = float(np.linalg.norm(mean, axis=1).mean())
    rms_rel_mean = rms / mean_norm if mean_norm != 0.0 else np.nan
    return rms, rms_rel_mean


def compute_upet_uncertainty(
    path: str,
    *,
    fmt: str = "lammps-dump-text",
    model: str = DEFAULT_MODEL,
    version: str = DEFAULT_VERSION,
    device: str = "auto",
    force_committee_models: list[str] | None = None,
) -> dict[str, np.ndarray]:
    device = get_device(device)

    print("building UPET calculator")
    calc = build_calculator(model, version, device)

    force_calcs = None
    if force_committee_models:
        print("building force committee calculators")
        force_calcs = [build_calculator(m, version, device) for m in force_committee_models]

    print("reading trajectory")
    frames = iread(path, index=":", format=fmt)

    energy_uq: list[float] = []
    energy_rms: list[float] = []
    force_rms: list[float] = []
    force_rms_rel_mean: list[float] = []

    for idx, atoms in enumerate(frames):
        print(f"getting frame {idx}")

        atoms.calc = calc
        energy_uq.append(energy_uncertainty(calc, atoms))
        energy_rms.append(energy_ensemble_rms(calc, atoms))
        atoms.calc = None

        if force_calcs is not None:
            rms, rms_rel = force_committee_rms(force_calcs, atoms)
            force_rms.append(rms)
            force_rms_rel_mean.append(rms_rel)

        if device == "cuda":
            torch.cuda.empty_cache()
        gc.collect()

    result = {
        "energy_uncertainty": np.asarray(energy_uq),
        "energy_ensemble_rms": np.asarray(energy_rms),
    }
    if force_calcs is not None:
        result["force_rms"] = np.asarray(force_rms)
        result["force_rms_rel_mean"] = np.asarray(force_rms_rel_mean)

    return result


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="UPET uncertainty / committee scoring")
    parser.add_argument("path", nargs="?", default="dump.out", help="trajectory path")
    parser.add_argument("--format", default="lammps-dump-text", help="ASE input format")
    parser.add_argument("--model", default=DEFAULT_MODEL, help="UPET model name")
    parser.add_argument("--version", default=DEFAULT_VERSION, help="UPET model version")
    parser.add_argument("--device", choices=["auto", "cpu", "cuda"], default="auto")
    parser.add_argument(
        "--force-committee-models",
        nargs="+",
        help="optional UPET models for force RMS committee, e.g. pet-mad-xs pet-mad-s",
    )
    parser.add_argument("--out-prefix", default="upet", help="output file prefix")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    result = compute_upet_uncertainty(
        args.path,
        fmt=args.format,
        model=args.model,
        version=args.version,
        device=args.device,
        force_committee_models=args.force_committee_models,
    )

    for name, values in result.items():
        out = f"{args.out_prefix}_{name}.dat"
        np.savetxt(out, values)
        print(f"wrote {out}: {len(values)} frames")


if __name__ == "__main__":
    main()
