use {
    crate::{
        command_title,
        display::{human_output, json_output},
        loading,
        prelude::*,
        sui::get_read_only_nexus_client,
    },
    nexus_sdk::nexus::workflow::DagSnapshot,
    std::{collections::BTreeMap, fmt::Write as _},
};

pub(crate) async fn inspect_dag(dag_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting DAG {dag_id}");
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading published DAG interface...");
    let snapshot = client
        .workflow()
        .inspect_dag(dag_id)
        .await
        .map_err(NexusCliError::Nexus)?;
    progress.success();
    human_output(&render_dag(&snapshot));
    json_output(&dag_json(&snapshot).map_err(NexusCliError::Any)?)
}

/// Projects a [`DagSnapshot`] into readable JSON.
///
/// Stored byte names become validated UTF8 text for CLI output.
fn dag_json(snapshot: &DagSnapshot) -> AnyResult<serde_json::Value> {
    let vertex_meta_schemas = snapshot
        .vertex_meta_schemas
        .iter()
        .map(|(vertex, schema)| {
            schema
                .to_json_value()
                .map(|schema| (vertex, schema))
                .map_err(|error| {
                    anyhow!("Could not render MetaSchema for vertex '{vertex}': {error}")
                })
        })
        .collect::<AnyResult<BTreeMap<_, _>>>()?;

    Ok(json!({
        "dag_id": snapshot.dag_id,
        "vertex_count": snapshot.vertex_count,
        "entry_groups": snapshot.entry_groups,
        "vertex_meta_schemas": vertex_meta_schemas,
    }))
}

fn render_dag(snapshot: &DagSnapshot) -> String {
    let mut output = String::new();
    writeln!(output, "DAG              {}", snapshot.dag_id)
        .expect("writing to a String cannot fail");
    writeln!(output, "Vertices         {}", snapshot.vertex_count)
        .expect("writing to a String cannot fail");

    if snapshot.entry_groups.is_empty() {
        output.push_str("\nNo entry groups found.\n");
        return output;
    }

    for (name, vertices) in &snapshot.entry_groups {
        writeln!(output, "\nEntry group {name}").expect("writing to a String cannot fail");
        for (vertex, ports) in vertices {
            writeln!(output, "  {vertex:<16}{}", ports.join(", "))
                .expect("writing to a String cannot fail");
        }
        writeln!(output, "Input JSON  {}", input_template(vertices))
            .expect("writing to a String cannot fail");
    }
    output
}

fn input_template(vertices: &BTreeMap<String, Vec<String>>) -> String {
    let template = vertices
        .iter()
        .map(|(vertex, ports)| {
            let ports = ports
                .iter()
                .map(|port| (port, "<value>"))
                .collect::<BTreeMap<_, _>>();
            (vertex, ports)
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_string(&template).expect("a string input template always serializes")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::move_bindings::interface::meta_schema::{
            MetaSchema,
            OutputVariantSchema,
            PortSchema,
            ValueKind,
        },
        std::collections::BTreeMap,
    };

    #[test]
    fn human_output_explains_every_required_input() {
        let snapshot = DagSnapshot {
            dag_id: sui::types::Address::from_static("0xd"),
            vertex_count: 1,
            entry_groups: BTreeMap::from([(
                "_default_group".to_owned(),
                BTreeMap::from([(
                    "sum".to_owned(),
                    vec!["0".to_owned(), "1".to_owned(), "2".to_owned()],
                )]),
            )]),
            vertex_meta_schemas: BTreeMap::new(),
        };

        let output = render_dag(&snapshot);

        assert!(output.contains("Entry group _default_group"));
        assert!(output.contains("sum             0, 1, 2"));
        assert!(
            output.contains(r#"Input JSON  {"sum":{"0":"<value>","1":"<value>","2":"<value>"}}"#)
        );
    }

    #[test]
    fn json_output_uses_readable_meta_schema_names() {
        let snapshot = DagSnapshot {
            dag_id: sui::types::Address::from_static("0xd"),
            vertex_count: 1,
            entry_groups: BTreeMap::new(),
            vertex_meta_schemas: BTreeMap::from([(
                "sum".to_owned(),
                MetaSchema::new(
                    vec![PortSchema::new(b"numbers".to_vec(), true, ValueKind::Data)],
                    vec![OutputVariantSchema::new(
                        b"ok".to_vec(),
                        vec![PortSchema::new(b"total".to_vec(), false, ValueKind::Data)],
                    )],
                ),
            )]),
        };

        let value = dag_json(&snapshot).expect("valid DAG schema should project");

        assert_eq!(
            value["vertex_meta_schemas"]["sum"]["input_ports"][0]["port_name"],
            "numbers"
        );
        assert_eq!(
            value["vertex_meta_schemas"]["sum"]["output_variants"][0]["variant_name"],
            "ok"
        );
        assert_eq!(
            value["vertex_meta_schemas"]["sum"]["output_variants"][0]["ports"][0]["port_name"],
            "total"
        );
        assert!(!value["vertex_meta_schemas"]
            .to_string()
            .contains("[110,117,109"));
    }
}
