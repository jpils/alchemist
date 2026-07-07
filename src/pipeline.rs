pub(crate) mod runner;

use crate::types::{JobId, FinishedData};
use std::{path::{Path, PathBuf}, process::Command, time::Duration};
use anyhow::Result;

pub(crate) trait PipelineStep {
    fn name(&self) -> &str;
    fn submit(&self) -> Result<JobId>;
    fn resubmit_from_current_state(&self) -> Result<JobId> {
        unimplemented!("Will be added in the future")
    }
    fn on_completion(&self, job_state: &FinishedData) -> Result<()>;
}

pub(crate) struct Pipeline {
    steps: Vec<Box<dyn PipelineStep>>
}

pub(crate) struct PipelineCtx {
    pub(crate) project_dir: PathBuf,
    pub(crate) generation: u32,
    pub(crate) poll_interval: Duration,
    pub(crate) max_retries: u32
}

pub(crate) struct StepCtx {
    pub(crate) working_dir: PathBuf,
    pub(crate) setup_dir: PathBuf,
    pub(crate) template: PathBuf,
}

pub(crate) enum MdEngine {
    Lammps,
}

impl MdEngine {
    fn as_str(&self) -> &str {
        match self {
            Self::Lammps => "lammps"
        }
    }
}

pub(crate) enum DftCode {
    Vasp,
}

impl DftCode {
    fn as_str(&self) -> &str {
        match self {
            Self::Vasp => "vasp"
        }
    }
}

pub(crate) enum ModelBackend {
    Upet,
    N2p2
}

impl ModelBackend {
    fn as_str(&self) -> &str {
        match self {
            Self::Upet => "UPET",
            Self::N2p2 => "n2p2"
        }
    }
}

pub(crate) enum ObcMethod {
    Rrmsfd,
}

impl ObcMethod {
    fn as_str(&self) -> &str {
        match self {
            Self::Rrmsfd => "RRMSFD (relative root mean squared force deviation)",
        }
    }
}

pub(crate) struct MdStep {
    engine: MdEngine,
    ctx: StepCtx
}

impl PipelineStep for MdStep {
    fn name(&self) -> &str {
        self.engine.as_str()
    }

    fn submit(&self) -> Result<JobId> {
        todo!() 
    }

    fn on_completion(&self, job_state: &FinishedData) -> Result<()> {
        todo!()
    }
}

pub(crate) struct DftStep {
    dft_code: DftCode,
    ctx: StepCtx
}

impl PipelineStep for DftStep {
    fn name(&self) -> &str {
        self.dft_code.as_str()
    }

    fn submit(&self) -> Result<JobId> {
        todo!() 
    }

    fn on_completion(&self, job_state: &FinishedData) -> Result<()> {
        todo!()
    }
}

pub(crate) struct TrainingStep {
    model_backend: ModelBackend,
    ctx: StepCtx
}

impl PipelineStep for TrainingStep {
    fn name(&self) -> &str {
        self.model_backend.as_str()
    }

    fn submit(&self) -> Result<JobId> {
        todo!() 
    }

    fn on_completion(&self, job_state: &FinishedData) -> Result<()> {
        todo!()
    }
}

pub(crate) struct QbcStep {
    method: ObcMethod,
    ctx: StepCtx
}

impl PipelineStep for QbcStep {
    fn name(&self) -> &str {
        self.method.as_str()
    }

    fn submit(&self) -> Result<JobId> {
        todo!() 
    }

    fn on_completion(&self, job_state: &FinishedData) -> Result<()> {
        todo!()
    }
}

fn parse_command() -> Result<Command> {
    todo!()
}
