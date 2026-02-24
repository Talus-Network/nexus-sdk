use {
    super::SharedObjectRef,
    serde::{Deserialize, Serialize},
};

/// Onchain persisted interface configuration
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InterfacePackageConfig {
    pub shared_objects: Vec<SharedObjectRef>,
}

#[cfg(test)]
mod tests {
    use {super::*, crate::sui};

    #[test]
    fn interface_package_config_serde_roundtrip() {
        let config = InterfacePackageConfig {
            shared_objects: vec![SharedObjectRef::new_imm(sui::types::Address::ZERO)],
        };

        let json = serde_json::to_string(&config).unwrap();
        let decoded: InterfacePackageConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.shared_objects, config.shared_objects);
    }
}
