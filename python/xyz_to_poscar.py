import sys
from ase.io import read, write


def main():
    if len(sys.argv) < 3:
        print("Invalid arguments.", file=sys.stderr)
        sys.exit(1)

    mode = sys.argv[1]

    if mode == "count":
        if len(sys.argv) != 3:
            print(
                "Usage: xyz_to_poscar.py count <input.xyz>",
                file=sys.stderr,
            )
            sys.exit(1)

        input_path = sys.argv[2]

        try:
            configs = read(input_path, index=":")
            print(len(configs))
        except Exception as e:
            print(f"ASE Counting Error: {e}", file=sys.stderr)
            sys.exit(1)

    elif mode == "extract":
        if len(sys.argv) != 5:
            print(
                "Usage: xyz_to_poscar.py extract <input.xyz> <index> <output.POSCAR>",
                file=sys.stderr,
            )
            sys.exit(1)

        input_path = sys.argv[2]
        target_index = int(sys.argv[3])
        output_path = sys.argv[4]

        try:
            atoms = read(input_path, index=target_index)
            write(output_path, atoms, format="vasp")
        except Exception as e:
            print(
                f"ASE Conversion Error at index {target_index}: {e}",
                file=sys.stderr,
            )
            sys.exit(1)

    else:
        print(f"Unknown mode '{mode}'", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()