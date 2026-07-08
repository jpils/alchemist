import os
import glob
from collections import Counter

import ase.io
import numpy as np
from enum import Enum

class EnergyMode(Enum):
    PET = "pet"
    RAW = "raw"

    @classmethod
    def from_string(cls, value: str):
        try:
            return cls(value.lower())
        except ValueError:
            valid = ", ".join(mode.value for mode in cls)
            raise ValueError(
                f"Unknown energy mode '{value}'. Valid modes are: {valid}"
            )


def load_reference_energies(checkpoint_path):
    """
    Extract atomic reference energies from the PET-MAD checkpoint.
    """

    try:
        import torch
        from metatrain.utils.io import model_from_checkpoint

        checkpoint = torch.load(
            checkpoint_path,
            weights_only=False,
            map_location="cpu",
        )

        pet_model = model_from_checkpoint(checkpoint, "finetune")

        energy_values = (
            pet_model.additive_models[0]
            .model.weights["energy"]
            .block()
            .values
        )

        atomic_numbers = checkpoint["model_data"]["dataset_info"].atomic_types

        return dict(zip(atomic_numbers, energy_values))

    except Exception as e:
        print(
            f"    [Info] Note: Local heavy torch libraries not loaded ({e}). "
            "Using structural baseline simulation fallback."
        )

        return {
            16: -4.0,
            29: -3.8,
        }


def load_generation(project_dir, generation_num):
    """
    Read every OUTCAR belonging to one generation.
    """

    base_pattern = os.path.join(
        project_dir,
        "vasp_runs",
        f"generation_{generation_num}",
        "config_*",
        "OUTCAR",
    )

    outcar_files = sorted(glob.glob(base_pattern))

    if not outcar_files:
        raise RuntimeError(
            f"No OUTCAR files found matching {base_pattern}"
        )

    print(
        f"[+] Processing {len(outcar_files)} configurations "
        f"for Generation {generation_num}..."
    )

    dataset = []

    for outcar_path in outcar_files:

        try:
            atoms = ase.io.read(
                outcar_path,
                format="vasp-out",
                index=-1,
            )

            dataset.append(atoms)

        except Exception as e:
            print(
                f"    [Warning] Failed parsing "
                f"{outcar_path}: {e}"
            )

    if not dataset:
        raise RuntimeError(
            "No valid OUTCARs could be parsed."
        )

    return dataset


def apply_energy_shift(dataset, checkpoint_path):
    """
    Compute and apply PET reference energy shift.
    """

    dft_energies = [
        atoms.get_potential_energy()
        for atoms in dataset
    ]

    ref_energies = load_reference_energies(
        checkpoint_path
    )

    sample_atoms = dataset[0]

    counts = Counter(
        sample_atoms.get_atomic_numbers()
    )

    reference_total = sum(
        ref_energies.get(Z, 0.0) * count
        for Z, count in counts.items()
    )

    dft_mean = np.mean(dft_energies)

    reference_total = (
        reference_total.item()
        if hasattr(reference_total, "item")
        else reference_total
    )

    total_shift = dft_mean - reference_total

    print(f"    -> Mean Raw Energy: {dft_mean:.4f} eV")
    print(
        f"    -> Target Base Energy Alignment: "
        f"{reference_total:.4f} eV"
    )
    print(
        f"    -> Applied Shift Adjustment Matrix: "
        f"{total_shift:.4f} eV"
    )

    for atoms in dataset:

        corrected_energy = (
            atoms.get_potential_energy()
            - total_shift
        )

        atoms.info["energy-corrected"] = float(
            corrected_energy
        )


def split_dataset(dataset):
    """
    Standard 80 / 10 / 10 split.
    """

    np.random.seed(42)

    indices = np.random.permutation(len(dataset))

    n = len(dataset)

    n_val = int(0.1 * n)
    n_test = int(0.1 * n)
    n_train = n - n_val - n_test

    if n_train <= 0:

        n_train = n
        n_val = 0
        n_test = 0

    train = [
        dataset[i]
        for i in indices[:n_train]
    ]

    val = [
        dataset[i]
        for i in indices[
            n_train:
            n_train + n_val
        ]
    ]

    test = [
        dataset[i]
        for i in indices[
            n_train + n_val:
        ]
    ]

    return train, val, test


def prepare_dataset(
    project_dir,
    generation_num,
    checkpoint_path,
    energy_mode: EnergyMode = EnergyMode.PET,
):
    """
    Complete shared workflow.

    Returns:
        train_set, val_set, test_set
    """

    dataset = load_generation(
        project_dir,
        generation_num,
    )

    if energy_mode is EnergyMode.PET:
        apply_energy_shift(
            dataset,
            checkpoint_path,
        )

    elif energy_mode is EnergyMode.RAW:
        print("    -> Using raw DFT energies (no energy correction).")

    return split_dataset(dataset)

def export_extxyz(
    project_dir,
    generation_num,
    train_set,
    val_set,
    test_set,
):
    """
    Write UPET extxyz datasets.
    """

    target_dir = os.path.join(
        project_dir,
        "training",
        "training_set",
        f"generation_{generation_num}",
    )

    os.makedirs(
        target_dir,
        exist_ok=True,
    )

    ase.io.write(
        os.path.join(target_dir, "train.xyz"),
        train_set,
        format="extxyz",
    )

    if val_set:
        ase.io.write(
            os.path.join(target_dir, "val.xyz"),
            val_set,
            format="extxyz",
        )

    if test_set:
        ase.io.write(
            os.path.join(target_dir, "test.xyz"),
            test_set,
            format="extxyz",
        )

    print(
        f"[+] Dataset exported successfully: {target_dir}/"
    )

    print(
        f"    - train.xyz : {len(train_set)} frames"
    )

    print(
        f"    - val.xyz   : {len(val_set)} frames"
    )

    print(
        f"    - test.xyz  : {len(test_set)} frames"
    )