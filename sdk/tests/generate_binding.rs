//! On-demand regeneration of the committed Move-binding IR.
//!
//! This is the *network* half of the binding pipeline (see
//! `sdk/src/idents/generated/`): it fetches normalized Move package metadata
//! from a running Sui gRPC endpoint and persists it as committed JSON. The
//! *offline* half (rendering address-free `ModuleAndNameIdent` constants from
//! that JSON) happens deterministically in `build.rs`, so normal builds never
//! touch the network.
//!
//! It is `#[ignore]`d so it never runs in CI; invoke it deliberately against a
//! localnet that has the Nexus packages published (see `just regenerate-idents`):
//!
//! ```bash
//! NEXUS_BINDING_GRPC_URL=http://127.0.0.1:9000 \
//! NEXUS_BINDING_PACKAGES="primitives=0x..,interface=0x..,registry=0x..,workflow=0x..,scheduler=0x.." \
//!   cargo test -p nexus-sdk --test generate_binding -- --ignored --nocapture
//! ```
//!
//! Each `name=0xid` pair becomes `sdk/src/idents/generated/ir/<name>.json`.

#![cfg(feature = "sui_types")]

use {
    nexus_sdk::sui::{grpc::Client, types::Address},
    std::{path::Path, str::FromStr},
    sui_move_codegen::fetch_package,
};

/// Directory (relative to the crate manifest) that holds the committed IR.
const IR_DIR: &str = "src/idents/generated/ir";

#[tokio::test]
#[ignore = "network + a localnet with Nexus packages published; run via `just regenerate-idents`"]
async fn regenerate_binding_ir() {
    let grpc_url = std::env::var("NEXUS_BINDING_GRPC_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());

    let packages = std::env::var("NEXUS_BINDING_PACKAGES").expect(
        "set NEXUS_BINDING_PACKAGES to a comma-separated list of `name=0xpackageid` pairs \
         (e.g. \"primitives=0x..,workflow=0x..\")",
    );

    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(IR_DIR);
    std::fs::create_dir_all(&out_dir).expect("Failed to create IR output directory.");

    let mut client = Client::new(&grpc_url)
        .unwrap_or_else(|e| panic!("Failed to create gRPC client for {grpc_url}: {e}"));

    for entry in packages.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, id) = entry
            .split_once('=')
            .unwrap_or_else(|| panic!("Malformed package entry `{entry}`, expected `name=0xid`."));

        let package_id = Address::from_str(id.trim())
            .unwrap_or_else(|e| panic!("Invalid package id `{id}` for `{name}`: {e}"));

        let package = fetch_package(&mut client, package_id)
            .await
            .unwrap_or_else(|e| panic!("Failed to fetch package `{name}` ({id}): {e}"));

        let json = package
            .to_json_string()
            .unwrap_or_else(|e| panic!("Failed to serialize IR for `{name}`: {e}"));

        let path = out_dir.join(format!("{name}.json"));
        std::fs::write(&path, format!("{json}\n"))
            .unwrap_or_else(|e| panic!("Failed to write {}: {e}", path.display()));

        println!(
            "wrote {} ({} modules)",
            path.display(),
            package.modules.len()
        );
    }
}
