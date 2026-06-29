pub mod interface_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/interface_types.rs"));
}

pub mod primitives_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/primitives_types.rs"));
}

pub mod registry_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/registry_types.rs"));
}

pub mod scheduler_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/scheduler_types.rs"));
}

pub mod sui_framework_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/sui_framework_types.rs"));
}

pub mod workflow_types {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/workflow_types.rs"));
}
