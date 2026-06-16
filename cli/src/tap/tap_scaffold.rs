use super::*;

pub(crate) async fn scaffold_tap_skill(
    name: String,
    target: PathBuf,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scaffolding TAP skill '{name}' in '{target}'",
        target = target.display()
    );

    let root = target.join(name.to_case(Case::Kebab));
    let package_name = name.to_case(Case::Snake);
    let module_name = package_name.clone();

    let handle = loading!("Writing TAP template files...");
    let files = scaffold_files(&name, &package_name, &module_name);

    create_dir_all(&root).await.map_err(NexusCliError::Io)?;

    for (path, contents) in files {
        let path = root.join(path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await.map_err(NexusCliError::Io)?;
        }

        // Use `tokio::fs::write` so each file is flushed and closed before we
        // move on — a dropped `tokio::fs::File` does not flush its buffer, so a
        // subsequent reader (e.g. `validate-skill`) could see a partial file.
        tokio::fs::write(path, contents.as_bytes())
            .await
            .map_err(NexusCliError::Io)?;
    }

    handle.success();
    notify_success!("Created TAP skill scaffold at {}", root.display());
    json_output(&scaffold_result_json(&root))?;

    Ok(())
}

pub(crate) fn scaffold_files(
    name: &str,
    package_name: &str,
    module_name: &str,
) -> Vec<(PathBuf, String)> {
    vec![
        (
            PathBuf::from("tap/Move.toml"),
            format!(
                r#"[package]
name = "{package_name}"
version = "1.0.0"
edition = "2024"

# Stage these four packages under `deps/<name>/` before running
# `tap publish-skill`. The default vertex-tool stub only imports
# `nexus_primitives`, but the full standard-TAP surface (cap-gated
# authorization, scheduled tasks, registry interactions) needs the other
# three. Drop unused entries if you want a leaner build.
[dependencies]
nexus_primitives = {{ local = "deps/primitives" }}
nexus_interface  = {{ local = "deps/interface" }}
nexus_registry   = {{ local = "deps/registry" }}
nexus_workflow   = {{ local = "deps/workflow" }}

# Map env alias → chain id for every Sui network you plan to publish to. The
# alias must match the `sui client envs` name and the value is what
# `sui client chain-identifier` prints while that env is active. The default
# entry below targets public testnet; if you target a different network add
# or replace the row, e.g. `mainnet = "35834a8a"`.
[environments]
testnet = "4c78adac"
"#
            ),
        ),
        (
            PathBuf::from(format!("tap/sources/{module_name}.move")),
            format!(
                r#"module {package_name}::{module_name};

/// Minimal third-party TAP package scaffold.
/// Fill this package with business logic, endpoint state, and standard TAP exports.
public struct {witness} has drop {{}}

public fun init_for_test(): {witness} {{
    {witness} {{}}
}}
"#,
                witness = name.to_case(Case::Pascal)
            ),
        ),
        (
            PathBuf::from("dag.json"),
            r#"{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.weather_skill@1"
      },
      "name": "entry",
      "entry_ports": [
        {
          "name": "input"
        }
      ]
    }
  ],
  "edges": []
}
"#
            .to_string(),
        ),
        (
            PathBuf::from("skill.tap.json"),
            format!(
                r#"{{
  "name": "{name}",
  "tap_package_name": "{package_name}",
  "dag_path": "dag.json",
  "tap_package_path": "tap",
  "requirements": {{
    "input_schema_commitment": [1],
    "payment_policy": "UserFunded",
    "schedule_policy": {{
      "recurrence": "Once",
      "allow_recursive": false
    }},
    "fixed_tools": []
  }},
  "shared_objects": [],
  "interface_revision": 1
}}
"#
            ),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_files_use_kebab_snake_and_expected_package_layout() {
        let files = scaffold_files("Weather Skill", "weather_skill", "weather_skill");
        let paths = files.iter().map(|(path, _)| path).collect::<Vec<_>>();

        assert!(paths.contains(&&PathBuf::from("tap/Move.toml")));
        assert!(paths.contains(&&PathBuf::from("tap/sources/weather_skill.move")));
        assert!(paths.contains(&&PathBuf::from("dag.json")));
        assert!(paths.contains(&&PathBuf::from("skill.tap.json")));
        assert!(files
            .iter()
            .any(|(_, contents)| contents.contains("module weather_skill::weather_skill;")));
    }

    #[test]
    fn scaffolded_move_toml_declares_all_four_nexus_dependencies() {
        // The scaffold ships with all four published Nexus packages
        // (primitives, interface, registry, workflow) declared so authors
        // who reach for any standard-TAP surface beyond the minimal vertex
        // tool — cap-gated authorization, scheduler interactions, registry
        // lookups — don't have to discover and add deps mid-build. Authors
        // who want a leaner manifest can drop unused entries themselves.
        let files = scaffold_files(
            "tutorial transfer",
            "tutorial_transfer",
            "tutorial_transfer",
        );
        let move_toml = files
            .iter()
            .find_map(|(path, contents)| {
                (path == &PathBuf::from("tap/Move.toml")).then_some(contents.as_str())
            })
            .expect("Move.toml present");
        for dep in [
            "nexus_primitives = { local = \"deps/primitives\" }",
            "nexus_interface  = { local = \"deps/interface\" }",
            "nexus_registry   = { local = \"deps/registry\" }",
            "nexus_workflow   = { local = \"deps/workflow\" }",
        ] {
            assert!(
                move_toml.contains(dep),
                "scaffolded Move.toml missing entry: {dep}"
            );
        }
    }

    #[test]
    fn scaffolded_move_toml_prefills_testnet_chain_id() {
        // The scaffold defaults to public testnet so `tap publish-skill`
        // works out of the box for the most common dev target. Authors who
        // target a different network edit the [environments] table.
        let files = scaffold_files(
            "tutorial transfer",
            "tutorial_transfer",
            "tutorial_transfer",
        );
        let move_toml = files
            .iter()
            .find_map(|(path, contents)| {
                (path == &PathBuf::from("tap/Move.toml")).then_some(contents.as_str())
            })
            .expect("Move.toml present");
        assert!(
            move_toml.contains("testnet = \"4c78adac\""),
            "scaffolded Move.toml must default [environments].testnet to the public testnet chain id"
        );
    }

    #[test]
    fn scaffolded_move_toml_uses_new_style_layout() {
        // `validate-skill` requires the new-style 2024 layout: `version`,
        // `edition = "2024"`, an `[environments]` table, and no `[addresses]`
        // (which would mark the package as old-style and break dependency
        // resolution against new-style published deps). Pin the scaffold to
        // that shape so the very first `validate-skill` after `tap scaffold`
        // doesn't reject the manifest it just wrote.
        let files = scaffold_files(
            "tutorial transfer",
            "tutorial_transfer",
            "tutorial_transfer",
        );
        let move_toml = files
            .iter()
            .find_map(|(path, contents)| {
                (path == &PathBuf::from("tap/Move.toml")).then_some(contents.as_str())
            })
            .expect("Move.toml present");

        assert!(
            move_toml.contains("version = \"1.0.0\""),
            "scaffolded Move.toml must carry a [package].version field"
        );
        assert!(
            move_toml.contains("edition = \"2024\""),
            "scaffolded Move.toml must use edition = \"2024\""
        );
        assert!(
            !move_toml.contains("2024.beta"),
            "scaffolded Move.toml must not use the old 2024.beta edition"
        );
        assert!(
            move_toml.contains("[environments]"),
            "scaffolded Move.toml must declare an [environments] table"
        );
        assert!(
            !move_toml.contains("[addresses]"),
            "scaffolded Move.toml must not declare [addresses] (old-style marker)"
        );
    }
}
