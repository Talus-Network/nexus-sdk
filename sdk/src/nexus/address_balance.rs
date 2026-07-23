//! Builds address balance gas transactions and allocates sender nonces.

use {
    crate::{
        nexus::error::NexusError,
        sui::{self, traits::*},
    },
    std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

/// Network context required by an address balance gas payment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubmissionContext {
    /// Current reference gas price.
    pub reference_gas_price: u64,
    /// Current epoch used to bound transaction validity.
    pub epoch: u64,
    /// Chain identifier used for cross chain replay protection.
    pub chain: sui::types::Digest,
}

/// Nonce authority for one address balance sender.
///
/// Every clone allocates from the same lock free sequence.
#[derive(Clone, Debug)]
pub struct NonceAllocator {
    next: Arc<AtomicU32>,
}

impl Default for NonceAllocator {
    fn default() -> Self {
        Self::new(0)
    }
}

impl NonceAllocator {
    /// Creates an allocator whose first result is `initial`.
    pub fn new(initial: u32) -> Self {
        Self {
            next: Arc::new(AtomicU32::new(initial)),
        }
    }

    /// Allocates the next nonce without allowing the counter to wrap.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Configuration`] when the sequence is exhausted.
    pub fn allocate(&self) -> Result<u32, NexusError> {
        self.next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| {
                NexusError::Configuration(
                    "address balance transaction nonce space is exhausted".into(),
                )
            })
    }
}

/// Fetches the current [`SubmissionContext`] for a new transaction.
///
/// # Errors
///
/// Returns [`NexusError`] when the RPC request fails or the chain identifier
/// cannot be parsed.
pub async fn fetch_submission_context(
    client: &mut sui::grpc::Client,
) -> Result<SubmissionContext, NexusError> {
    let epoch_request =
        sui::grpc::GetEpochRequest::latest().with_read_mask(sui::grpc::FieldMask::from_paths([
            "epoch",
            "reference_gas_price",
        ]));
    let epoch_response = client
        .ledger_client()
        .get_epoch(epoch_request)
        .await
        .map_err(|error| NexusError::Rpc(error.into()))?
        .into_inner();
    let epoch = epoch_response.epoch();

    let service_info = client
        .ledger_client()
        .get_service_info(sui::grpc::GetServiceInfoRequest::default())
        .await
        .map_err(|error| NexusError::Rpc(error.into()))?
        .into_inner();
    let chain = service_info
        .chain_id
        .ok_or_else(|| NexusError::Parsing(anyhow::anyhow!("service info is missing chain id")))?
        .parse::<sui::types::Digest>()
        .map_err(|error| NexusError::Parsing(error.into()))?;

    Ok(SubmissionContext {
        reference_gas_price: epoch.reference_gas_price(),
        epoch: epoch.epoch(),
        chain,
    })
}

/// Finishes a [`sui::types::ProgrammableTransaction`] with address balance
/// based gas.
///
/// The returned [`sui::types::Transaction`] has no gas objects and is valid
/// during the current epoch and the following epoch from [`SubmissionContext`].
pub fn finish_transaction(
    ptb: sui::types::ProgrammableTransaction,
    sender: sui::types::Address,
    gas_budget: u64,
    context: SubmissionContext,
    nonce: u32,
) -> sui::types::Transaction {
    sui::types::Transaction {
        kind: sui::types::TransactionKind::ProgrammableTransaction(ptb),
        sender,
        gas_payment: sui::types::GasPayment {
            objects: Vec::new(),
            owner: sender,
            price: context.reference_gas_price,
            budget: gas_budget,
        },
        expiration: sui::types::TransactionExpiration::ValidDuring {
            min_epoch: Some(context.epoch),
            max_epoch: Some(context.epoch.saturating_add(1)),
            min_timestamp: None,
            max_timestamp: None,
            chain: context.chain,
            nonce,
        },
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{collections::HashSet, thread},
    };

    #[test]
    fn finishes_with_address_balance_gas_and_validity_context() {
        let sender = sui::types::Address::from_static("0x42");
        let chain = sui::types::Digest::from([7; 32]);
        let context = SubmissionContext {
            reference_gas_price: 1_234,
            epoch: 17,
            chain,
        };
        let ptb = sui::types::ProgrammableTransaction {
            inputs: Vec::new(),
            commands: Vec::new(),
        };

        let tx = finish_transaction(ptb, sender, 99_000, context, 41);

        assert!(tx.gas_payment.objects.is_empty());
        assert_eq!(tx.gas_payment.owner, sender);
        assert_eq!(tx.gas_payment.price, 1_234);
        assert_eq!(tx.gas_payment.budget, 99_000);
        assert_eq!(
            tx.expiration,
            sui::types::TransactionExpiration::ValidDuring {
                min_epoch: Some(17),
                max_epoch: Some(18),
                min_timestamp: None,
                max_timestamp: None,
                chain,
                nonce: 41,
            }
        );
    }

    #[test]
    fn cloned_allocators_issue_one_shared_nonce_sequence() {
        let allocator = NonceAllocator::default();
        let handles = (0..8)
            .map(|_| {
                let allocator = allocator.clone();
                thread::spawn(move || {
                    (0..32)
                        .map(|_| allocator.allocate().unwrap())
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();

        let nonces = handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<HashSet<_>>();

        assert_eq!(nonces.len(), 256);
        assert_eq!(nonces.iter().copied().min(), Some(0));
        assert_eq!(nonces.iter().copied().max(), Some(255));
    }

    #[test]
    fn nonce_allocator_rejects_exhaustion() {
        let allocator = NonceAllocator::new(u32::MAX - 1);

        assert_eq!(allocator.allocate().unwrap(), u32::MAX - 1);
        let error = allocator.allocate().unwrap_err();

        assert!(error.to_string().contains("nonce space is exhausted"));
    }
}
