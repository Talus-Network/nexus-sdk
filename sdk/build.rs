use std::fs;

fn main() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("Should find Cargo.toml");
    let parsed: toml::Value = cargo_toml.parse().expect("Cargo.toml should valid");

    let sui_sdk_tag = &parsed["dependencies"]["sui_package_management"]["tag"]
        .as_str()
        .expect("Should read dependencies.sui_package_management tag");

    println!("cargo:rustc-env=SUI_SDK_TAG={sui_sdk_tag}");
}
