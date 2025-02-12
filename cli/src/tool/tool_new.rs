use {
    crate::{command_title, display::*, prelude::*},
    std::path::Path,
    tokio::{fs::File, io::AsyncWriteExt},
};

/// Available templates for tool generation.
#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum ToolTemplate {
    Rust,
}

/// Create a new tool based on the provided name and template.
pub(crate) async fn create_new_tool(
    name: String,
    template: ToolTemplate,
    target: String,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating a new Nexus Tool '{name}' with template '{template:?}' in '{target}'");

    // Join the target directory and the tool name.
    let path = Path::new(&target).join(&name);

    // Create a dummy file for now.
    let mut file = match File::create(path).await {
        Ok(file) => file,
        Err(e) => {
            return Err(NexusCliError::IoError(e));
        }
    };

    if let Err(e) = file
        .write_all(
            format!(
                "Amazing new tool: {} with template: {:?} in {}",
                name, template, target
            )
            .as_bytes(),
        )
        .await
    {
        return Err(NexusCliError::IoError(e));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[tokio::test]
    async fn test_create_new_tool() {
        let result =
            create_new_tool("test".to_string(), ToolTemplate::Rust, "/tmp".to_string()).await;

        assert_matches!(result, Ok(()));

        // Check that file was written to `/tmp/test` with the correct contents.
        let path = Path::new("/tmp").join("test");
        let contents = tokio::fs::read_to_string(path).await.unwrap();

        assert_eq!(
            contents,
            "Amazing new tool: test with template: Rust in /tmp"
        );
    }
}
