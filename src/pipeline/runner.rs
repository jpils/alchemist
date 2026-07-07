use anyhow::{Result, anyhow};
use crate::{types::{FinalJobStatus, FinishedData}, watcher::wait_for_job};

use super::{Pipeline, PipelineCtx};

pub(crate) trait Runner {
    fn run(&self, pipeline: &Pipeline, pipeline_ctx: &PipelineCtx) -> Result<()>;
}

pub(crate) struct SlurmRunner;

impl Runner for SlurmRunner {
    fn run(&self, pipeline: &Pipeline, pipeline_ctx: &PipelineCtx) -> Result<()> {
        for step in pipeline.steps.iter() {
            let mut job_id = step.submit()?;
            let mut retry_count = 0;

            while retry_count < pipeline_ctx.max_retries {
                let finished_data = wait_for_job(&job_id, pipeline_ctx.poll_interval)?;

                match finished_data.final_status {
                    FinalJobStatus::Completed => {
                        step.on_completion(&finished_data)?;
                        break
                    },
                    FinalJobStatus::Timeout => { 
                        job_id = step.resubmit_from_current_state()?;
                        retry_count += 1;
                    },
                    FinalJobStatus::OutOfMemory => return Err(anyhow!("Out of memory")),
                    FinalJobStatus::Cancelled => return Err(anyhow!("Job cancelled")),
                    FinalJobStatus::Failed => return Err(anyhow!("Job failed")),
                    FinalJobStatus::Other(e) => return Err(anyhow!("Unknown status: {e}"))
                }
            }
        }

        Ok(())
    } 
}

pub(crate) struct LocalRunner;

impl Runner for LocalRunner {
    fn run(&self, pipeline: &Pipeline, pipeline_ctx: &PipelineCtx) -> Result<()> {
        todo!()
    }
}

pub(crate) struct TestRunner;

impl Runner for TestRunner {
    fn run(&self, pipeline: &Pipeline, pipeline_ctx: &PipelineCtx) -> Result<()> {
         todo!()
    } 
}

pub(crate) struct DryRunner;

impl Runner for DryRunner {
    fn run(&self, pipeline: &Pipeline, pipeline_ctx: &PipelineCtx) -> Result<()> {
        todo!()
    }
}
