// Scheduler transaction builders for WASM
// Added in v0.5.0 to support on-chain task management
//
// Note: SDK's native transaction builders (nexus_sdk::transactions::scheduler)
// cannot be used in WASM due to sui-crypto and sui-transaction-builder dependencies.
// This module produces JSON command structures that JavaScript can convert to Sui transactions.
//
// IMPORTANT: This implementation mirrors sdk/src/transactions/scheduler.rs exactly.

use {
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
    wasm_bindgen::prelude::*,
};

// =============================================================================
// Constants
// =============================================================================

const SUI_FRAMEWORK: &str = "0x2";
const MOVE_STDLIB: &str = "0x1";

// =============================================================================
// Public Types (WASM-exported)
// =============================================================================

/// Generator type for scheduler tasks
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorKind {
    Queue    = 0,
    Periodic = 1,
}

/// Task state actions
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum TaskStateAction {
    Pause  = 0,
    Resume = 1,
    Cancel = 2,
}

// =============================================================================
// Input Parameter Types
// =============================================================================

/// Scheduler task creation parameters
#[derive(Serialize, Deserialize)]
pub struct CreateTaskParams {
    pub dag_id: String,
    pub entry_group: String,
    pub input_data: serde_json::Value,
    pub metadata: Vec<(String, String)>,
    pub execution_priority_fee_per_gas_unit: u64,
    pub generator: String, // "queue" or "periodic"
    pub network_id: String, // Required for execution policy
}

/// Occurrence request parameters
#[derive(Serialize, Deserialize)]
pub struct OccurrenceParams {
    pub start_ms: Option<u64>,
    pub deadline_ms: Option<u64>,
    pub start_offset_ms: Option<u64>,
    pub deadline_offset_ms: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

/// Periodic schedule configuration
#[derive(Serialize, Deserialize)]
pub struct PeriodicConfig {
    pub first_start_ms: u64,
    pub period_ms: u64,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

// =============================================================================
// Result Types (WASM-exported)
// =============================================================================

#[wasm_bindgen]
pub struct SchedulerResult {
    is_success: bool,
    error_message: Option<String>,
    transaction_data: Option<String>,
}

#[wasm_bindgen]
impl SchedulerResult {
    #[wasm_bindgen(getter)]
    pub fn is_success(&self) -> bool {
        self.is_success
    }

    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn transaction_data(&self) -> Option<String> {
        self.transaction_data.clone()
    }
}

impl SchedulerResult {
    fn ok(data: String) -> Self {
        Self {
            is_success: true,
            error_message: None,
            transaction_data: Some(data),
        }
    }

    fn err(msg: impl Into<String>) -> Self {
        Self {
            is_success: false,
            error_message: Some(msg.into()),
            transaction_data: None,
        }
    }
}

// =============================================================================
// Internal: Command Builder
// =============================================================================

/// Helper for building PTB commands in JSON format
struct CommandBuilder {
    commands: Vec<serde_json::Value>,
    result_index: usize,
    workflow_pkg: String,
    primitives_pkg: String,
}

impl CommandBuilder {
    fn new(nexus_objects: &HashMap<String, String>) -> Self {
        Self {
            commands: Vec::new(),
            result_index: 0,
            workflow_pkg: nexus_objects
                .get("workflow_pkg_id")
                .cloned()
                .unwrap_or_else(|| "{{workflow_pkg_id}}".into()),
            primitives_pkg: nexus_objects
                .get("primitives_pkg_id")
                .cloned()
                .unwrap_or_else(|| "{{primitives_pkg_id}}".into()),
        }
    }

    /// Add a move call command and return its result index
    fn move_call(
        &mut self,
        target: impl Into<String>,
        type_args: Vec<String>,
        arguments: Vec<serde_json::Value>,
    ) -> usize {
        let idx = self.result_index;
        self.commands.push(serde_json::json!({
            "type": "moveCall",
            "target": target.into(),
            "typeArguments": type_args,
            "arguments": arguments,
            "result_index": idx
        }));
        self.result_index += 1;
        idx
    }

    /// Reference a previous result
    fn result(index: usize) -> serde_json::Value {
        serde_json::json!({"type": "result", "index": index})
    }

    /// Pure value argument
    fn pure(pure_type: &str, value: impl Serialize) -> serde_json::Value {
        serde_json::json!({"type": "pure", "pure_type": pure_type, "value": value})
    }

    /// Shared object argument
    fn shared_object(id: &str, mutable: bool) -> serde_json::Value {
        serde_json::json!({"type": "shared_object_by_id", "id": id, "mutable": mutable})
    }

    /// Clock object argument
    fn clock() -> serde_json::Value {
        serde_json::json!({"type": "clock_object"})
    }

    // =========================================================================
    // Sui Framework Helpers
    // =========================================================================

    /// Create empty VecMap<K, V>
    fn vec_map_empty(&mut self, key_type: &str, value_type: &str) -> usize {
        self.move_call(
            format!("{SUI_FRAMEWORK}::vec_map::empty"),
            vec![key_type.into(), value_type.into()],
            vec![],
        )
    }

    /// Insert into VecMap
    fn vec_map_insert(
        &mut self,
        key_type: &str,
        value_type: &str,
        map_idx: usize,
        key_idx: usize,
        value_idx: usize,
    ) {
        self.move_call(
            format!("{SUI_FRAMEWORK}::vec_map::insert"),
            vec![key_type.into(), value_type.into()],
            vec![
                Self::result(map_idx),
                Self::result(key_idx),
                Self::result(value_idx),
            ],
        );
    }

    /// Create empty TableVec<T>
    fn table_vec_empty(&mut self, elem_type: &str) -> usize {
        self.move_call(
            format!("{SUI_FRAMEWORK}::table_vec::empty"),
            vec![elem_type.into()],
            vec![],
        )
    }

    /// Push back to TableVec
    fn table_vec_push_back(&mut self, elem_type: &str, vec_idx: usize, elem_idx: usize) {
        self.move_call(
            format!("{SUI_FRAMEWORK}::table_vec::push_back"),
            vec![elem_type.into()],
            vec![Self::result(vec_idx), Self::result(elem_idx)],
        );
    }

    /// Drop TableVec
    fn table_vec_drop(&mut self, elem_type: &str, vec_idx: usize) {
        self.move_call(
            format!("{SUI_FRAMEWORK}::table_vec::drop"),
            vec![elem_type.into()],
            vec![Self::result(vec_idx)],
        );
    }

    /// Create string from bytes
    fn string_utf8(&mut self, value: &str) -> usize {
        self.move_call(
            format!("{MOVE_STDLIB}::string::utf8"),
            vec![],
            vec![Self::pure("vector_u8", value.as_bytes().to_vec())],
        )
    }

    /// Share object publicly
    fn public_share_object(&mut self, type_tag: &str, obj_idx: usize) {
        self.move_call(
            format!("{SUI_FRAMEWORK}::transfer::public_share_object"),
            vec![type_tag.into()],
            vec![Self::result(obj_idx)],
        );
    }

    // =========================================================================
    // Workflow Package Helpers
    // =========================================================================

    fn workflow_target(&self, module: &str, function: &str) -> String {
        format!("{}::{}::{}", self.workflow_pkg, module, function)
    }

    fn primitives_target(&self, module: &str, function: &str) -> String {
        format!("{}::{}::{}", self.primitives_pkg, module, function)
    }

    /// Create witness symbol from type
    fn witness_symbol(&mut self, witness_type: &str) -> usize {
        self.move_call(
            self.primitives_target("policy", "witness_symbol"),
            vec![witness_type.into()],
            vec![],
        )
    }

    /// dag::vertex_from_string
    fn vertex_from_string(&mut self, name: &str) -> usize {
        self.move_call(
            self.workflow_target("dag", "vertex_from_string"),
            vec![],
            vec![Self::pure("string", name)],
        )
    }

    /// dag::input_port_from_string
    fn input_port_from_string(&mut self, name: &str) -> usize {
        self.move_call(
            self.workflow_target("dag", "input_port_from_string"),
            vec![],
            vec![Self::pure("string", name)],
        )
    }

    /// dag::entry_group_from_string
    fn entry_group_from_string(&mut self, name: &str) -> usize {
        self.move_call(
            self.workflow_target("dag", "entry_group_from_string"),
            vec![],
            vec![Self::pure("string", name)],
        )
    }

    /// data::inline_one (create NexusData)
    fn nexus_data_inline(&mut self, json_value: &serde_json::Value) -> anyhow::Result<usize> {
        let json_bytes = serde_json::to_string(json_value)?.into_bytes();
        Ok(self.move_call(
            self.primitives_target("data", "inline_one"),
            vec![],
            vec![Self::pure("vector_u8", json_bytes)],
        ))
    }

    // =========================================================================
    // Build Final Output
    // =========================================================================

    fn build(self) -> Vec<serde_json::Value> {
        self.commands
    }
}

// =============================================================================
// Input Data Processing
// =============================================================================

/// Build the nested VecMap structure for input data
/// VecMap<Vertex, VecMap<InputPort, NexusData>>
fn build_input_data_commands(
    cb: &mut CommandBuilder,
    input_data: &serde_json::Value,
) -> anyhow::Result<usize> {
    let vertex_type = format!("{}::dag::Vertex", cb.workflow_pkg);
    let port_type = format!("{}::dag::InputPort", cb.workflow_pkg);
    let data_type = format!("{}::data::NexusData", cb.primitives_pkg);
    let inner_map_type = format!("{SUI_FRAMEWORK}::vec_map::VecMap<{port_type}, {data_type}>");

    // Outer VecMap<Vertex, VecMap<InputPort, NexusData>>
    let outer_map = cb.vec_map_empty(&vertex_type, &inner_map_type);

    if let Some(vertices) = input_data.as_object() {
        for (vertex_name, ports) in vertices {
            let vertex_idx = cb.vertex_from_string(vertex_name);
            let inner_map = cb.vec_map_empty(&port_type, &data_type);

            if let Some(port_obj) = ports.as_object() {
                for (port_name, port_value) in port_obj {
                    let port_idx = cb.input_port_from_string(port_name);
                    let data_idx = cb.nexus_data_inline(port_value)?;
                    cb.vec_map_insert(&port_type, &data_type, inner_map, port_idx, data_idx);
                }
            }

            cb.vec_map_insert(
                &vertex_type,
                &inner_map_type,
                outer_map,
                vertex_idx,
                inner_map,
            );
        }
    }

    Ok(outer_map)
}

/// Build metadata VecMap<String, String>
fn build_metadata_commands(cb: &mut CommandBuilder, metadata: &[(String, String)]) -> usize {
    let string_type = format!("{MOVE_STDLIB}::string::String");
    let metadata_map = cb.vec_map_empty(&string_type, &string_type);

    for (key, value) in metadata {
        let key_idx = cb.string_utf8(key);
        let value_idx = cb.string_utf8(value);
        cb.vec_map_insert(&string_type, &string_type, metadata_map, key_idx, value_idx);
    }

    metadata_map
}

/// Build constraints policy with proper witness/TableVec pattern
/// Mirrors SDK: new_constraints_policy()
fn build_constraints_policy(
    cb: &mut CommandBuilder,
    generator: &str,
) -> usize {
    let symbol_type = format!("{}::policy::Symbol", cb.primitives_pkg);
    
    // 1. Create witness symbol based on generator type
    let witness_type = if generator == "periodic" {
        format!("{}::scheduler::PeriodicGeneratorWitness", cb.workflow_pkg)
    } else {
        format!("{}::scheduler::QueueGeneratorWitness", cb.workflow_pkg)
    };
    let constraint_symbol = cb.witness_symbol(&witness_type);

    // 2. Create TableVec<Symbol> and push the symbol
    let constraint_sequence = cb.table_vec_empty(&symbol_type);
    cb.table_vec_push_back(&symbol_type, constraint_sequence, constraint_symbol);

    // 3. Create constraints policy from sequence
    let constraints = cb.move_call(
        cb.workflow_target("scheduler", "new_constraints_policy"),
        vec![],
        vec![CommandBuilder::result(constraint_sequence)],
    );

    // 4. Drop the TableVec (it's consumed)
    cb.table_vec_drop(&symbol_type, constraint_sequence);

    // 5. Create and register generator state
    if generator == "periodic" {
        // Create periodic generator state
        let periodic_state = cb.move_call(
            cb.workflow_target("scheduler", "new_periodic_generator_state"),
            vec![],
            vec![],
        );
        // Register it
        cb.move_call(
            cb.workflow_target("scheduler", "register_periodic_generator"),
            vec![],
            vec![CommandBuilder::result(constraints), CommandBuilder::result(periodic_state)],
        );
    } else {
        // Create queue generator state
        let queue_state = cb.move_call(
            cb.workflow_target("scheduler", "new_queue_generator_state"),
            vec![],
            vec![],
        );
        // Register it
        cb.move_call(
            cb.workflow_target("scheduler", "register_queue_generator"),
            vec![],
            vec![CommandBuilder::result(constraints), CommandBuilder::result(queue_state)],
        );
    }

    constraints
}

/// Build execution policy with proper witness/TableVec pattern
/// Mirrors SDK: new_execution_policy()
fn build_execution_policy(
    cb: &mut CommandBuilder,
    dag_id: &str,
    network_id: &str,
    entry_group: &str,
    input_data: &serde_json::Value,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<usize> {
    let symbol_type = format!("{}::policy::Symbol", cb.primitives_pkg);

    // 1. Create witness symbol for BeginDagExecutionWitness
    let witness_type = format!("{}::default_tap::BeginDagExecutionWitness", cb.workflow_pkg);
    let execution_symbol = cb.witness_symbol(&witness_type);

    // 2. Create TableVec<Symbol> and push the symbol
    let execution_sequence = cb.table_vec_empty(&symbol_type);
    cb.table_vec_push_back(&symbol_type, execution_sequence, execution_symbol);

    // 3. Create execution policy from sequence
    let execution = cb.move_call(
        cb.workflow_target("scheduler", "new_execution_policy"),
        vec![],
        vec![CommandBuilder::result(execution_sequence)],
    );

    // 4. Drop the TableVec (it's consumed)
    cb.table_vec_drop(&symbol_type, execution_sequence);

    // 5. Create dag_id argument (ID from object_id)
    let dag_id_arg = cb.move_call(
        format!("{SUI_FRAMEWORK}::object::id_from_address"),
        vec![],
        vec![CommandBuilder::pure("address", dag_id)],
    );

    // 6. Create network_id argument (ID from object_id)
    let network_id_arg = cb.move_call(
        format!("{SUI_FRAMEWORK}::object::id_from_address"),
        vec![],
        vec![CommandBuilder::pure("address", network_id)],
    );

    // 7. Create entry_group argument
    let entry_group_arg = cb.entry_group_from_string(entry_group);

    // 8. Build input data VecMap
    let with_vertex_inputs = build_input_data_commands(cb, input_data)?;

    // 9. Create DAG execution config
    let config = cb.move_call(
        cb.workflow_target("dag", "new_dag_execution_config"),
        vec![],
        vec![
            CommandBuilder::result(dag_id_arg),
            CommandBuilder::result(network_id_arg),
            CommandBuilder::result(entry_group_arg),
            CommandBuilder::result(with_vertex_inputs),
            CommandBuilder::pure("u64", priority_fee_per_gas_unit),
        ],
    );

    // 10. Register the config on execution policy
    cb.move_call(
        cb.workflow_target("default_tap", "register_begin_execution"),
        vec![],
        vec![CommandBuilder::result(execution), CommandBuilder::result(config)],
    );

    Ok(execution)
}

// =============================================================================
// WASM-Exported Functions
// =============================================================================

/// Build transaction for creating a scheduler task
/// CLI: `nexus scheduler task create`
#[wasm_bindgen]
pub fn build_scheduler_task_create_transaction(
    params_json: &str,
    nexus_objects_json: &str,
) -> SchedulerResult {
    match build_task_create_impl(params_json, nexus_objects_json) {
        Ok(data) => SchedulerResult::ok(data),
        Err(e) => SchedulerResult::err(format!("Failed to build task create transaction: {e}")),
    }
}

fn build_task_create_impl(params_json: &str, nexus_objects_json: &str) -> anyhow::Result<String> {
    let params: CreateTaskParams = serde_json::from_str(params_json)?;
    let nexus_objects: HashMap<String, String> = serde_json::from_str(nexus_objects_json)?;

    let mut cb = CommandBuilder::new(&nexus_objects);

    // 1. Build metadata
    let metadata_map = build_metadata_commands(&mut cb, &params.metadata);
    let metadata_arg = cb.move_call(
        cb.workflow_target("scheduler", "new_metadata"),
        vec![],
        vec![CommandBuilder::result(metadata_map)],
    );

    // 2. Build constraints policy (with witness/TableVec pattern)
    let constraints_arg = build_constraints_policy(&mut cb, &params.generator);

    // 3. Build execution policy (with witness/TableVec pattern + config + register)
    let execution_arg = build_execution_policy(
        &mut cb,
        &params.dag_id,
        &params.network_id,
        &params.entry_group,
        &params.input_data,
        params.execution_priority_fee_per_gas_unit,
    )?;

    // 4. Create task
    let task_idx = cb.move_call(
        cb.workflow_target("scheduler", "new"),
        vec![],
        vec![
            CommandBuilder::result(metadata_arg),
            CommandBuilder::result(constraints_arg),
            CommandBuilder::result(execution_arg),
        ],
    );

    // 5. Share task object
    let task_type = format!("{}::scheduler::Task", cb.workflow_pkg);
    cb.public_share_object(&task_type, task_idx);

    Ok(serde_json::to_string(&serde_json::json!({
        "operation": "scheduler_task_create",
        "commands": cb.build(),
        "dag_id": params.dag_id,
        "network_id": params.network_id,
        "entry_group": params.entry_group,
        "generator": params.generator,
        "priority_fee_per_gas_unit": params.execution_priority_fee_per_gas_unit
    }))?)
}

/// Build transaction for adding an occurrence to a task
/// CLI: `nexus scheduler occurrence add`
#[wasm_bindgen]
pub fn build_scheduler_occurrence_add_transaction(
    task_id: &str,
    params_json: &str,
    nexus_objects_json: &str,
) -> SchedulerResult {
    match build_occurrence_add_impl(task_id, params_json, nexus_objects_json) {
        Ok(data) => SchedulerResult::ok(data),
        Err(e) => SchedulerResult::err(format!("Failed to build occurrence add transaction: {e}")),
    }
}

fn build_occurrence_add_impl(
    task_id: &str,
    params_json: &str,
    nexus_objects_json: &str,
) -> anyhow::Result<String> {
    let params: OccurrenceParams = serde_json::from_str(params_json)?;
    let nexus_objects: HashMap<String, String> = serde_json::from_str(nexus_objects_json)?;

    let mut cb = CommandBuilder::new(&nexus_objects);
    let is_absolute = params.start_ms.is_some();

    if is_absolute {
        // Absolute time occurrence
        let start_ms = params.start_ms.unwrap();
        let deadline_offset = params
            .deadline_offset_ms
            .or_else(|| params.deadline_ms.map(|d| d.saturating_sub(start_ms)));

        cb.move_call(
            cb.workflow_target("scheduler", "add_occurrence_absolute_for_task"),
            vec![],
            vec![
                CommandBuilder::shared_object(task_id, true),
                CommandBuilder::pure("u64", start_ms),
                CommandBuilder::pure("option_u64", deadline_offset),
                CommandBuilder::pure("u64", params.priority_fee_per_gas_unit),
                CommandBuilder::clock(),
            ],
        );
    } else {
        // Relative time occurrence
        let start_offset = params.start_offset_ms.unwrap_or(0);

        cb.move_call(
            cb.workflow_target("scheduler", "add_occurrence_relative_for_task"),
            vec![],
            vec![
                CommandBuilder::shared_object(task_id, true),
                CommandBuilder::pure("u64", start_offset),
                CommandBuilder::pure("option_u64", params.deadline_offset_ms),
                CommandBuilder::pure("u64", params.priority_fee_per_gas_unit),
                CommandBuilder::clock(),
            ],
        );
    }

    Ok(serde_json::to_string(&serde_json::json!({
        "operation": "scheduler_occurrence_add",
        "commands": cb.build(),
        "task_id": task_id,
        "is_absolute": is_absolute,
        "priority_fee_per_gas_unit": params.priority_fee_per_gas_unit
    }))?)
}

/// Build transaction for configuring periodic scheduling
/// CLI: `nexus scheduler periodic set`
#[wasm_bindgen]
pub fn build_scheduler_periodic_set_transaction(
    task_id: &str,
    config_json: &str,
    nexus_objects_json: &str,
) -> SchedulerResult {
    match build_periodic_set_impl(task_id, config_json, nexus_objects_json) {
        Ok(data) => SchedulerResult::ok(data),
        Err(e) => SchedulerResult::err(format!("Failed to build periodic set transaction: {e}")),
    }
}

fn build_periodic_set_impl(
    task_id: &str,
    config_json: &str,
    nexus_objects_json: &str,
) -> anyhow::Result<String> {
    let config: PeriodicConfig = serde_json::from_str(config_json)?;
    let nexus_objects: HashMap<String, String> = serde_json::from_str(nexus_objects_json)?;

    let mut cb = CommandBuilder::new(&nexus_objects);

    // SDK does NOT use clock for this function
    // Arguments: task, first_start_ms, period_ms, deadline_offset_ms, max_iterations, priority_fee
    cb.move_call(
        cb.workflow_target("scheduler", "new_or_modify_periodic_for_task"),
        vec![],
        vec![
            CommandBuilder::shared_object(task_id, true),
            CommandBuilder::pure("u64", config.first_start_ms),
            CommandBuilder::pure("u64", config.period_ms),
            CommandBuilder::pure("option_u64", config.deadline_offset_ms),
            CommandBuilder::pure("option_u64", config.max_iterations),
            CommandBuilder::pure("u64", config.priority_fee_per_gas_unit),
        ],
    );

    Ok(serde_json::to_string(&serde_json::json!({
        "operation": "scheduler_periodic_set",
        "commands": cb.build(),
        "task_id": task_id,
        "first_start_ms": config.first_start_ms,
        "period_ms": config.period_ms
    }))?)
}

/// Build transaction for disabling periodic scheduling
/// CLI: `nexus scheduler periodic disable`
#[wasm_bindgen]
pub fn build_scheduler_periodic_disable_transaction(
    task_id: &str,
    nexus_objects_json: &str,
) -> SchedulerResult {
    match build_periodic_disable_impl(task_id, nexus_objects_json) {
        Ok(data) => SchedulerResult::ok(data),
        Err(e) => {
            SchedulerResult::err(format!("Failed to build periodic disable transaction: {e}"))
        }
    }
}

fn build_periodic_disable_impl(task_id: &str, nexus_objects_json: &str) -> anyhow::Result<String> {
    let nexus_objects: HashMap<String, String> = serde_json::from_str(nexus_objects_json)?;
    let mut cb = CommandBuilder::new(&nexus_objects);

    cb.move_call(
        cb.workflow_target("scheduler", "disable_periodic_for_task"),
        vec![],
        vec![CommandBuilder::shared_object(task_id, true)],
    );

    Ok(serde_json::to_string(&serde_json::json!({
        "operation": "scheduler_periodic_disable",
        "commands": cb.build(),
        "task_id": task_id
    }))?)
}

/// Build transaction for changing task state (pause/resume/cancel)
/// CLI: `nexus scheduler task pause/resume/cancel`
#[wasm_bindgen]
pub fn build_scheduler_task_state_transaction(
    task_id: &str,
    action: TaskStateAction,
    nexus_objects_json: &str,
) -> SchedulerResult {
    match build_task_state_impl(task_id, action, nexus_objects_json) {
        Ok(data) => SchedulerResult::ok(data),
        Err(e) => SchedulerResult::err(format!("Failed to build task state transaction: {e}")),
    }
}

fn build_task_state_impl(
    task_id: &str,
    action: TaskStateAction,
    nexus_objects_json: &str,
) -> anyhow::Result<String> {
    let nexus_objects: HashMap<String, String> = serde_json::from_str(nexus_objects_json)?;
    let mut cb = CommandBuilder::new(&nexus_objects);

    let action_name = match action {
        TaskStateAction::Pause => "pause",
        TaskStateAction::Resume => "resume",
        TaskStateAction::Cancel => "cancel",
    };

    cb.move_call(
        cb.workflow_target("scheduler", action_name),
        vec![],
        vec![CommandBuilder::shared_object(task_id, true)],
    );

    Ok(serde_json::to_string(&serde_json::json!({
        "operation": format!("scheduler_task_{action_name}"),
        "commands": cb.build(),
        "task_id": task_id,
        "action": action_name
    }))?)
}
