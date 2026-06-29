import numpy as np
from ase.io import read, write
from scipy.signal import find_peaks

def convert_configs(ids: np.ndarray, shift: bool = True):
    dw_atom_ids = [3, 6, 10, 13, 17, 20, 24, 27, 31, 34, 38, 41, 45, 48, 52, 55, 98, 102, 105, 109, 112, 116, 119, 123, 126, 130, 133, 137, 140, 144, 147, 151, 194, 199, 211, 218, 236, 248, 255, 265, 270, 275, 287, 294, 312, 324, 331, 341]
    for i in ids:
        current_config = read("../run/dump.out", index = i, format = "lammps-dump-text")
        print(f"writing config {i}")

        if shift:
            atoms = current_config.get_positions()
            for id in dw_atom_ids:
                shift_vec = 2*np.random.rand(3)-1
                shift_vec[0] *= 0.01 # x direction
                shift_vec[1:] *= 0.035 # y z direction
                print(f"Config {i} before shift: {atoms[id]}")
                atoms[id] += shift_vec 
                print(f"Config {i} after shift: {atoms[id]}")
                print(f"shift: {shift_vec}")

            current_config.set_positions(atoms)
            write(f"single_point_configs/POSCAR.{i}", current_config, format = "vasp")
        else:
            write(f"single_point_configs/POSCAR.{i}", current_config, format = "vasp")

    np.savetxt("config_ids", ids)
    return

def main():
    percentile = 0.1
    exclude = 0
    N_configs = 12
    data = np.loadtxt('rms_rel_mean.dat')
    threshold = np.quantile(data[exclude:], 1.0 - percentile)
    print(threshold)

    peaks_ids, _ = find_peaks(data, distance=100, height=(threshold, None))
    peaks_data = data[peaks_ids]
    peaks_sorted_ids = np.argsort(peaks_data)[::-1]
    peaks_ids = peaks_ids[peaks_sorted_ids][:N_configs]
    peaks_data = peaks_data[peaks_sorted_ids][:N_configs]
    peaks = np.column_stack((peaks_ids, peaks_data))
    np.savetxt("peaks", peaks)

    print(peaks_ids)
    convert_configs(peaks_ids)
    
    return

if __name__ == "__main__":
    main()
