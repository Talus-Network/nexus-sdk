//! Regenerates the committed Move-binding IR.
//!
//! This is the *network* half of the binding pipeline (see `sdk/src/idents/`):
//! it fetches normalized Move package metadata from a running Sui gRPC endpoint
//! and persists it as committed JSON. The *offline* half (rendering address-free
//! identifier constants from that JSON) happens deterministically in `build.rs`,
//! so normal builds never touch the network.
//!
//! Run it deliberately against a node that exposes the target packages (a
//! localnet with the Nexus packages published, plus the `0x1`/`0x2` framework
//! packages every node carries). `just sdk regenerate-idents` wraps the whole
//! pipeline:
//!
//! ```bash
//! NEXUS_BINDING_GRPC_URL=http://127.0.0.1:9000 \
//! NEXUS_BINDING_PACKAGES="primitives=0x..,interface=0x..,workflow=0x..,move_std=0x1,sui_framework=0x2" \
//!   cargo run -p nexus-sdk --features binding_codegen --bin generate_binding
//! ```
//!
//! Each `name=0xid` pair becomes `sdk/src/idents/generated/ir/<name>.json`.

use {
    nexus_sdk::sui::{grpc::Client, types::Address},
    std::{path::Path, process::ExitCode, str::FromStr},
    sui_move_codegen::fetch_package,
};

/// Directory (relative to the crate manifest) that holds the committed IR.
const IR_DIR: &str = "src/idents/generated/ir";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let grpc_url = std::env::var("NEXUS_BINDING_GRPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());

    let packages = std::env::var("NEXUS_BINDING_PACKAGES").map_err(|_| {
        "set NEXUS_BINDING_PACKAGES to a comma-separated list of `name=0xpackageid` pairs \
         (e.g. \"primitives=0x..,move_std=0x1,sui_framework=0x2\")"
            .to_string()
    })?;

    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(IR_DIR);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("create {}: {e}", out_dir.display()))?;

    let mut client =
        Client::new(&grpc_url).map_err(|e| format!("gRPC client for {grpc_url}: {e}"))?;

    for entry in packages.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, id) = entry
            .split_once('=')
            .ok_or_else(|| format!("malformed package entry `{entry}`, expected `name=0xid`"))?;

        let package_id =
            Address::from_str(id.trim()).map_err(|e| format!("invalid package id `{id}`: {e}"))?;

        let package = fetch_package(&mut client, package_id)
            .await
            .map_err(|e| format!("fetch `{name}` ({id}): {e}"))?;

        let json = package
            .to_json_string()
            .map_err(|e| format!("serialize IR for `{name}`: {e}"))?;

        let path = out_dir.join(format!("{name}.json"));
        std::fs::write(&path, format!("{json}\n"))
            .map_err(|e| format!("write {}: {e}", path.display()))?;

        println!(
            "wrote {} ({} modules)",
            path.display(),
            package.modules.len()
        );
    }

    Ok(())
}
