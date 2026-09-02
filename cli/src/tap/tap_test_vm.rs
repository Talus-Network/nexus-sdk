//! Sui VM configuration used by TAP unit tests.

use {
    move_unit_test::vm_test_setup::VMTestSetup,
    move_vm_config::runtime::VMConfig,
    move_vm_runtime::natives::extensions::NativeContextExtensions,
    std::{
        cell::RefCell,
        collections::BTreeMap,
        ops::{Deref, DerefMut},
        rc::Rc,
        sync::{Arc, LazyLock},
    },
    sui_adapter::gas_meter::SuiGasMeter,
    sui_move_natives::{
        object_runtime::ObjectRuntime,
        scratch::ScratchRuntime,
        test_scenario::InMemoryTestStore,
        transaction_context::TransactionContext,
        NativesCostTable,
    },
    sui_protocol_config::ProtocolConfig,
    sui_types::{
        base_types::{SuiAddress, TxContext},
        digests::TransactionDigest,
        gas::{SuiGasStatus, SuiGasStatusAPI},
        gas_model::{tables::GasStatus, units_types::Gas},
        in_memory_storage::InMemoryStorage,
        metrics::ExecutionMetrics,
    },
};

/// Maximum instruction budget used by Sui Move unit tests.
pub(super) static MAX_UNIT_TEST_INSTRUCTIONS: LazyLock<u64> =
    LazyLock::new(|| ProtocolConfig::get_for_max_version_UNSAFE().max_tx_gas());

const TEST_GAS_PRICE: u64 = 500;

/// Configures Move unit tests with the same natives and object runtime as Sui.
pub(super) struct SuiVMTestSetup {
    gas_price: u64,
    reference_gas_price: u64,
    protocol_config: ProtocolConfig,
    native_function_table: move_vm_runtime::natives::functions::NativeFunctionTable,
}

impl SuiVMTestSetup {
    pub(super) fn new() -> Self {
        let protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
        let native_function_table = sui_move_natives::all_natives(false, &protocol_config);
        Self {
            gas_price: TEST_GAS_PRICE,
            reference_gas_price: TEST_GAS_PRICE,
            protocol_config,
            native_function_table,
        }
    }
}

/// Retains test object state while one native call context is active.
pub(super) struct SuiExtensionsBuilder<'a> {
    store: InMemoryTestStore,
    protocol_config: &'a ProtocolConfig,
}

impl VMTestSetup for SuiVMTestSetup {
    type ExtensionsBuilder<'a> = SuiExtensionsBuilder<'a>;
    type Meter<'a> = SuiGasMeter<SuiGasStatusTestWrapper>;

    fn new_meter<'a>(&'a self, execution_bound: Option<u64>) -> Self::Meter<'a> {
        SuiGasMeter(SuiGasStatusTestWrapper(
            SuiGasStatus::new(
                execution_bound.unwrap_or(*MAX_UNIT_TEST_INSTRUCTIONS),
                self.gas_price,
                self.reference_gas_price,
                &self.protocol_config,
            )
            .expect("the unit test gas configuration is valid"),
        ))
    }

    fn used_gas<'a>(&'a self, execution_bound: u64, meter: Self::Meter<'a>) -> u64 {
        Gas::new(execution_bound)
            .checked_sub(meter.0.remaining_gas())
            .expect("used gas does not exceed the test bound")
            .into()
    }

    fn vm_config(&self) -> VMConfig {
        sui_adapter::adapter::vm_config(&self.protocol_config)
    }

    fn native_function_table(&self) -> move_vm_runtime::natives::functions::NativeFunctionTable {
        self.native_function_table.clone()
    }

    fn new_extensions_builder(&self) -> SuiExtensionsBuilder<'_> {
        SuiExtensionsBuilder {
            store: InMemoryTestStore(RefCell::new(InMemoryStorage::default())),
            protocol_config: &self.protocol_config,
        }
    }

    fn new_native_context_extensions<'a, 'ext>(
        &'a self,
        builder: &'ext SuiExtensionsBuilder<'a>,
    ) -> NativeContextExtensions<'ext> {
        let mut extensions = NativeContextExtensions::default();
        let registry = prometheus::Registry::new();
        let metrics = Arc::new(ExecutionMetrics::new(&registry));
        let protocol_config = builder.protocol_config;
        extensions.add(ObjectRuntime::new(
            &builder.store,
            BTreeMap::new(),
            false,
            protocol_config,
            metrics,
            0,
        ));
        extensions.add(NativesCostTable::from_protocol_config(protocol_config));
        extensions.add(ScratchRuntime::new(protocol_config));
        let context = TxContext::new_from_components(
            &SuiAddress::ZERO,
            &TransactionDigest::default(),
            &0,
            0,
            0,
            0,
            0,
            None,
            &self.protocol_config,
        );
        extensions.add(TransactionContext::new_for_testing(Rc::new(RefCell::new(
            context,
        ))));
        extensions.add(&builder.store);
        extensions
    }
}

/// Adapts Sui gas accounting to the unit test runner meter trait.
pub(super) struct SuiGasStatusTestWrapper(SuiGasStatus);

impl Deref for SuiGasStatusTestWrapper {
    type Target = GasStatus;

    fn deref(&self) -> &Self::Target {
        self.0.move_gas_status()
    }
}

impl DerefMut for SuiGasStatusTestWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.move_gas_status_mut()
    }
}
