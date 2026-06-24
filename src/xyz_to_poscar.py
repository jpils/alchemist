import sys
from ase.io import read, write

def main():
    if len(sys.argv) < 3:
        print("Invalid arguments passed to Python subsystem.", file=sys.stderr)
        sys.exit(1)
    
    mode = sys.argv[1]
    input_path = sys.argv[2]
    
    if mode == "count":
        try:
            # index=':' reads the entire file into a list of configurations
            configs = read(input_path, index=':')
            print(len(configs)) # Output the total count to stdout for Rust to capture
        except Exception as e:
            print(f"ASE Counting Error: {e}", file=sys.stderr)
            sys.exit(1)
            
    elif mode == "convert":
        if len(sys.argv) != 5:
            print("Usage: python3 xyz_to_poscar.py convert <input> <output> <index>", file=sys.stderr)
            sys.exit(1)
            
        output_path = sys.argv[3]
        target_index = int(sys.argv[4])
        
        try:
            # Read only the single requested configuration index
            atoms = read(input_path, index=target_index)
            write(output_path, atoms, format='vasp')
        except Exception as e:
            print(f"ASE Conversion Error at index {target_index}: {e}", file=sys.stderr)
            sys.exit(1)

if __name__ == "__main__":
    main()