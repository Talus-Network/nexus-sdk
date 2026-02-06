use {
    std::path::Path,
    sui_move_call::SharedMoveObject,
    sui_move_codegen::{
        fetch_package,
        render::{render_package, RenderOptions},
        Address,
        Client,
    },
    sui_move_ptb::ptb,
};

#[tokio::test]
async fn test_generate_move_bindings() {
    let mut client = Client::new("https://grpc.ssfn.devnet.production.taluslabs.dev").unwrap();
    let package_id =
        Address::from_static("0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06");

    let package = fetch_package(&mut client, package_id).await.unwrap();

    let render = render_package(&package, &RenderOptions::default());

    tokio::fs::write(
        Path::new("/home/kouks/Code/talus/nexus-sdk/sdk/tests/workflow.rs"),
        render,
    )
    .await
    .unwrap();
}

// cargo run -p sui-move-codegen --example localnet_workspace -- 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f --check --grpc https://grpc.ssfn.devnet.production.taluslabs.dev --out nexus
// cargo run -p sui-move-codegen --example localnet_workspace -- 0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88 --check --grpc https://grpc.ssfn.devnet.production.taluslabs.dev --out nexus
// cargo run -p sui-move-codegen --example localnet_workspace -- 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06 --check --grpc https://grpc.ssfn.devnet.production.taluslabs.dev --external 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f=nexus/move_pkg_d8f40e14a26960f5 --external 0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88=nexus/move_pkg_d749533b00b57ed7 --out nexus
