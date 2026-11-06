//! Signing seam for Sui transactions. [`WalletClient`](super::wallet::WalletClient)
//! signs through this trait, so a key may live in process memory or behind a remote service.

use {
    crate::{
        nexus::error::NexusError,
        sui::{self, traits::*},
    },
    futures::future::{BoxFuture, FutureExt as _},
};

/// Produces Sui user signatures for transactions sent by [`Self::address`].
pub trait TransactionSigner: Send + Sync + std::fmt::Debug {
    /// The Sui address that transactions signed by this signer are sent from.
    fn address(&self) -> sui::types::Address;

    /// Signs the transaction's signing digest.
    fn sign<'a>(
        &'a self,
        tx: &'a sui::types::Transaction,
    ) -> BoxFuture<'a, Result<sui::types::UserSignature, NexusError>>;
}

impl TransactionSigner for sui::crypto::Ed25519PrivateKey {
    fn address(&self) -> sui::types::Address {
        self.public_key().derive_address()
    }

    fn sign<'a>(
        &'a self,
        tx: &'a sui::types::Transaction,
    ) -> BoxFuture<'a, Result<sui::types::UserSignature, NexusError>> {
        futures::future::ready(
            self.sign_transaction(tx)
                .map_err(|error| NexusError::Wallet(error.into())),
        )
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {super::TransactionSigner, crate::sui};

    fn transaction(sender: sui::types::Address) -> sui::types::Transaction {
        crate::nexus::address_balance::finish_transaction(
            sui::types::ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            },
            sender,
            1_000_000,
            crate::nexus::address_balance::SubmissionContext {
                reference_gas_price: 1,
                epoch: 1,
                chain: sui::types::Digest::new([9; 32]),
            },
            1,
        )
    }

    #[tokio::test]
    async fn local_signature_is_byte_identical_to_sui_signer() {
        let key = sui::crypto::Ed25519PrivateKey::new([17; 32]);
        let tx = transaction(TransactionSigner::address(&key));
        let through_trait = TransactionSigner::sign(&key, &tx).await.unwrap();
        let direct = sui::crypto::SuiSigner::sign_transaction(&key, &tx).unwrap();
        assert_eq!(through_trait, direct);
    }

    #[test]
    fn local_address_matches_public_key() {
        let key = sui::crypto::Ed25519PrivateKey::new([17; 32]);
        assert_eq!(
            TransactionSigner::address(&key),
            key.public_key().derive_address()
        );
    }
}
