module scheduler_metadata_test::scheduler;

use std::string::String;
use sui::transfer;
use sui::vec_map::VecMap;

public struct Metadata has store, drop {
    values: VecMap<String, String>,
}

public struct Task has key {
    id: UID,
    metadata: Metadata,
}

public fun new_metadata(values: VecMap<String, String>): Metadata {
    Metadata { values }
}

public fun create_task(metadata: Metadata, ctx: &mut TxContext) {
    transfer::share_object(Task {
        id: object::new(ctx),
        metadata,
    });
}
