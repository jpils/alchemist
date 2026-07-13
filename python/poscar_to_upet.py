import sys

from dataset import (
    EnergyMode,
    prepare_dataset,
    split_dataset,
)

from formats import export_extxyz


def main():

    project_dir = (
        sys.argv[1]
        if len(sys.argv) > 1
        else "."
    )

    generation = (
        int(sys.argv[2])
        if len(sys.argv) > 2
        else 1
    )

    checkpoint = (
        sys.argv[3]
        if len(sys.argv) > 3
        else "pet-mad-xs-v1.5.0.ckpt"
    )

    energy_mode = EnergyMode.from_string(
        sys.argv[4]
        if len(sys.argv) > 4
        else "pet"
    )

    print(
        f"[+] Project directory: {project_dir}"
    )

    print(
        f"[+] Python processing engine received checkpoint: "
        f"{checkpoint}"
    )

    dataset = prepare_dataset(
        project_dir,
        generation,
        checkpoint,
        energy_mode,
    )

    train_set, validation_set, test_set = split_dataset(
        dataset,
    )

    export_extxyz(
        project_dir,
        generation,
        train_set,
        validation_set,
        test_set,
    )

if __name__ == "__main__":
    main()