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
    json_output(&snapshot)
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
    use {super::*, std::collections::BTreeMap};

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
}
