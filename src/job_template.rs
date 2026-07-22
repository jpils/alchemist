use std::fs;
use std::io;
use std::path::Path;

/// Render a generic user-provided template.
///
/// Each replacement key should be provided without braces:
///
///     ("checkpoint", "/path/to/model.ckpt")
///
/// replaces:
///
///     {{checkpoint}}
pub fn render_template(
    template_path: &Path,
    output_path: &Path,
    replacements: &[(&str, String)],
) -> io::Result<()> {
    let template = fs::read_to_string(template_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Failed to read template {}: {}",
                template_path.display(),
                error
            ),
        )
    })?;

    let mut rendered = template;

    for (key, value) in replacements {
        let placeholder = format!("{{{{{key}}}}}");

        if !rendered.contains(&placeholder) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Template {} is missing required placeholder {}",
                    template_path.display(),
                    placeholder
                ),
            ));
        }

        rendered = rendered.replace(&placeholder, value);
    }

    reject_unknown_placeholders(&rendered, template_path)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(output_path, rendered).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "Failed to write rendered template {}: {}",
                output_path.display(),
                error
            ),
        )
    })?;

    Ok(())
}

/// Render a Slurm job template.
///
/// Required placeholders:
/// - {{generation}}
/// - {{max_index}}
pub fn render_job_template(
    template_path: &Path,
    output_path: &Path,
    generation: u32,
    max_index: usize,
) -> io::Result<()> {
    render_template(
        template_path,
        output_path,
        &[
            ("generation", generation.to_string()),
            ("max_index", max_index.to_string()),
        ],
    )?;

    set_executable(output_path)?;

    Ok(())
}

fn reject_unknown_placeholders(rendered: &str, template_path: &Path) -> io::Result<()> {
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Template {} contains an unknown or malformed placeholder",
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
