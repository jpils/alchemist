use std::error::Error;
use std::path::PathBuf;
use std::process::Command;

pub fn scheduler_home() -> Result<PathBuf, Box<dyn Error>> {
    let base = dirs::data_local_dir()
        .ok_or("Could not determine local data directory")?;

    let scheduler = base.join("ai-scheduler");

    if scheduler.exists() {
        Ok(scheduler)
    } else {
        Err(
            "ai-scheduler is not initialized.\nRun `ai_scheduler init` first."
                .into(),
        )
    }
}

/// Construct a command that executes Python inside the scheduler's
/// Pixi environment.
pub fn pixi_python(environment: &str) -> Result<Command, Box<dyn Error>>{
    let scheduler = scheduler_home()?;

    let mut cmd = Command::new("pixi");

    cmd.current_dir(&scheduler);

    cmd.arg("run")
        .arg("-e")
        .arg(environment)
        .arg("--manifest-path")
        .arg(scheduler.join("pixi.toml"))
        .arg("python");

    Ok(cmd)
}