use std::fs;
use std::io;
use std::path::Path;

/// Render a user-provided job script template.
///
/// Supported placeholders:
/// - {{generation}}
/// - {{max_index}}
pub fn render_job_template(
    template_path: &Path,
    output_path: &Path,
    generation: u32,
    max_index: usize,
) -> io::Result<()> {
    let template = fs::read_to_string(template_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Failed to read job template {}: {}",
                template_path.display(),
                error
            ),
        )
    })?;

    validate_required_placeholders(&template, template_path)?;

    let rendered = template
        .replace("{{generation}}", &generation.to_string())
        .replace("{{max_index}}", &max_index.to_string());

    reject_unknown_placeholders(&rendered, template_path)?;

    fs::write(output_path, rendered).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Failed to write rendered job script {}: {}",
                output_path.display(),
                error
            ),
        )
    })?;

    set_executable(output_path)?;

    Ok(())
}

fn validate_required_placeholders(
    template: &str,
    template_path: &Path,
) -> io::Result<()> {
    for placeholder in ["{{generation}}", "{{max_index}}"] {
        if !template.contains(placeholder) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Job template {} is missing required placeholder {}",
                    template_path.display(),
                    placeholder
                ),
            ));
        }
    }

    Ok(())
}

fn reject_unknown_placeholders(
    rendered: &str,
    template_path: &Path,
) -> io::Result<()> {
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Job template {} contains an unknown or malformed placeholder",
                template_path.display()
            ),
        ));
    }

    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}