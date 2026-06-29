const UPET_UNCERTAINTY_PY: &str = include_str!("../python/upet_uncertainty.py");

use pyo3::prelude::*;

pub(crate) struct UpetSetup {
    model: String,
    version: String,
    device: Device
}

pub(crate) enum Device {
    Auto,
    CPU,
    Cuda,
}

pub(crate) fn calcluate_disagreement() {
    todo!()
}
