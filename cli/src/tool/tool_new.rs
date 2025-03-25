use {
    crate::{command_title, display::json_output, loading, prelude::*},
    convert_case::{Case, Casing},
    minijinja::{context, Environment},
    tokio::{
        fs::{create_dir_all, File},
        io::AsyncWriteExt,
    },
};

/// Available templates for tool generation.
#[derive(Clone, Debug, ValueEnum)]
pub(crate) enum ToolTemplate {
    Rust,
}

impl ToolTemplate {
    /// For each template, transform the template based on the given variables
    /// and return the files to write.
    pub(crate) fn transform(&self, name: &str) -> AnyResult<Vec<(String, Option<String>)>> {
        match self {
            ToolTemplate::Rust => {
                let name_kebab_case = name.to_case(Case::Kebab);
                let name_pascal_case = name.to_case(Case::Pascal);

                let mut env = Environment::new();

                env.add_template("main", include_str!("templates/rust/main.rs.jinja"))?;
                env.add_template("cargo", include_str!("templates/rust/Cargo.toml.jinja"))?;

                let main_template = env.get_template("main")?;
                let cargo_template = env.get_template("cargo")?;

                Ok(vec![
                    ("src".to_string(), None),
                    (
                        "src/main.rs".to_string(),
                        Some(main_template.render(context! { name_kebab_case, name_pascal_case })?),
                    ),
                    (
                        "Cargo.toml".to_string(),
                        Some(
                            cargo_template
                                .render(context! { name_kebab_case, name_pascal_case })?,
                        ),
                    ),
                ])
            }
        }
    }
}

/// Create a new tool based on the provided name and template.
pub(crate) async fn create_new_tool(
    name: String,
    template: ToolTemplate,
    target: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating a new Nexus Tool '{name}' with template '{template:?}' in '{target}'",
        target = target.display()
    );

    let transforming_template = loading!("Transforming template...");

    let files = match template.transform(&name) {
        Ok(files) => files,
        Err(e) => {
            transforming_template.error();

            return Err(NexusCliError::Any(e));
        }
    };

    transforming_template.success();

    // Write each file that was generated by the template.
    let writing_file = loading!("Writing template files...");

    // Create the tool's root directory.
    let root_directory = target.join(&name);

    if let Err(e) = create_dir_all(root_directory).await {
        writing_file.error();

        return Err(NexusCliError::Io(e));
    };

    for (path, content) in files {
        let path = target.join(&name).join(&path);

        // Check if we need to create a directory.
        let content = match content {
            Some(content) => content,
            None => {
                if let Err(e) = create_dir_all(path).await {
                    writing_file.error();

                    return Err(NexusCliError::Io(e));
                }

                continue;
            }
        };

        let mut file = match File::create(path).await {
            Ok(file) => file,
            Err(e) => {
                writing_file.error();

                return Err(NexusCliError::Io(e));
            }
        };

        if let Err(e) = file.write_all(content.as_bytes()).await {
            writing_file.error();

            return Err(NexusCliError::Io(e));
        }
    }

    writing_file.success();

    json_output(&json!({}))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[tokio::test]
    async fn test_create_new_tool() {
        let result = create_new_tool(
            "test".to_string(),
            ToolTemplate::Rust,
            PathBuf::from("/tmp/nexus-tool"),
        )
        .await;

        assert_matches!(result, Ok(()));

        // Check that file was written to `/tmp/nexus-tool/test/src/main.rs` with the correct contents.
        let path = Path::new("/tmp/nexus-tool").join("test/src/main.rs");
        let contents = tokio::fs::read_to_string(path).await.unwrap();

        assert!(contents.contains("domain.author.test@1"));
        assert!(contents.contains("http://localhost:8080"));
        assert!(contents.contains("struct Test;"));
        assert!(contents.contains("impl NexusTool for Test {"));

        // Check that file was written to `/tmp/test/Cargo.toml` with the correct contents.
        let path = Path::new("/tmp/nexus-tool").join("test/Cargo.toml");
        let contents = tokio::fs::read_to_string(path).await.unwrap();

        assert!(contents.contains(r#"name = "test""#));
        assert!(contents.contains("[dependencies.nexus-toolkit]"));

        // Remove any leftover artifacts.
        tokio::fs::remove_dir_all("/tmp/nexus-tool").await.unwrap();
    }
}
