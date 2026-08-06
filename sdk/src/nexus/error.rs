//! Common error types for Nexus-related functionality.

use {std::time::Duration, thiserror::Error};

/// Durable transaction state known when execution does not complete normally.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionErrorState {
    /// The network rejected the transaction before execution.
    SubmissionRejected,
    /// The client cannot prove whether the transaction reached the network.
    SubmissionUnknown,
    /// The transaction executed, but checkpoint confirmation is unknown.
    ConfirmationUnknown,
    /// The transaction was confirmed with failed effects.
    ExecutionFailed,
}

/// A transaction failure that preserves every fact needed for recovery.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransactionError {
    /// The transaction was rejected before execution.
    #[error("transaction {digest} was rejected before execution: {source}")]
    SubmissionRejected {
        digest: crate::sui::types::Digest,
        #[source]
        source: tonic::Status,
    },
    /// The transaction submission result is unknown.
    #[error("transaction {digest} submission state is unknown: {source}")]
    SubmissionUnknown {
        digest: crate::sui::types::Digest,
        #[source]
        source: tonic::Status,
    },
    /// The transaction executed, but did not reach a checkpoint in time.
    #[error(
        "transaction {digest} was executed but checkpoint confirmation is unknown after {timeout:?}"
    )]
    ConfirmationTimedOut {
        digest: crate::sui::types::Digest,
        timeout: Duration,
        response: Box<crate::sui::grpc::ExecuteTransactionResponse>,
    },
    /// The transaction executed, but checkpoint observation failed.
    #[error("transaction {digest} was executed but checkpoint confirmation failed: {source}")]
    ConfirmationFailed {
        digest: crate::sui::types::Digest,
        response: Box<crate::sui::grpc::ExecuteTransactionResponse>,
        #[source]
        source: tonic::Status,
    },
    /// The confirmation response omitted required transaction data.
    #[error("transaction {digest} confirmation response is invalid: {message}")]
    ConfirmationResponseInvalid {
        digest: crate::sui::types::Digest,
        response: Box<crate::sui::grpc::ExecuteTransactionResponse>,
        message: &'static str,
    },
    /// The transaction was confirmed with failed effects.
    #[error(
        "transaction {digest} failed at checkpoint {checkpoint} in command {command:?}: {error:?}"
    )]
    ExecutionFailed {
        digest: crate::sui::types::Digest,
        checkpoint: u64,
        error: crate::sui::types::ExecutionError,
        command: Option<u64>,
    },
}

impl TransactionError {
    /// Creates a [`TransactionError::SubmissionRejected`] error.
    pub fn submission_rejected(digest: crate::sui::types::Digest, source: tonic::Status) -> Self {
        Self::SubmissionRejected { digest, source }
    }

    /// Creates a [`TransactionError::SubmissionUnknown`] error.
    pub fn submission_unknown(digest: crate::sui::types::Digest, source: tonic::Status) -> Self {
        Self::SubmissionUnknown { digest, source }
    }

    /// Creates a [`TransactionError::ConfirmationTimedOut`] error.
    pub fn confirmation_timed_out(
        digest: crate::sui::types::Digest,
        timeout: Duration,
        response: crate::sui::grpc::ExecuteTransactionResponse,
    ) -> Self {
        Self::ConfirmationTimedOut {
            digest,
            timeout,
            response: Box::new(response),
        }
    }

    /// Creates a [`TransactionError::ConfirmationFailed`] error.
    pub fn confirmation_failed(
        digest: crate::sui::types::Digest,
        response: crate::sui::grpc::ExecuteTransactionResponse,
        source: tonic::Status,
    ) -> Self {
        Self::ConfirmationFailed {
            digest,
            response: Box::new(response),
            source,
        }
    }

    /// Creates a [`TransactionError::ConfirmationResponseInvalid`] error.
    pub fn confirmation_response_invalid(
        digest: crate::sui::types::Digest,
        response: crate::sui::grpc::ExecuteTransactionResponse,
        message: &'static str,
    ) -> Self {
        Self::ConfirmationResponseInvalid {
            digest,
            response: Box::new(response),
            message,
        }
    }

    /// Creates a [`TransactionError::ExecutionFailed`] error.
    pub fn execution_failed(
        digest: crate::sui::types::Digest,
        checkpoint: u64,
        error: crate::sui::types::ExecutionError,
        command: Option<u64>,
    ) -> Self {
        Self::ExecutionFailed {
            digest,
            checkpoint,
            error,
            command,
        }
    }

    /// Returns the transaction digest.
    pub fn digest(&self) -> &crate::sui::types::Digest {
        match self {
            Self::SubmissionRejected { digest, .. }
            | Self::SubmissionUnknown { digest, .. }
            | Self::ConfirmationTimedOut { digest, .. }
            | Self::ConfirmationFailed { digest, .. }
            | Self::ConfirmationResponseInvalid { digest, .. }
            | Self::ExecutionFailed { digest, .. } => digest,
        }
    }

    /// Returns the known transaction state.
    pub fn state(&self) -> TransactionErrorState {
        match self {
            Self::SubmissionRejected { .. } => TransactionErrorState::SubmissionRejected,
            Self::SubmissionUnknown { .. } => TransactionErrorState::SubmissionUnknown,
            Self::ConfirmationTimedOut { .. }
            | Self::ConfirmationFailed { .. }
            | Self::ConfirmationResponseInvalid { .. } => {
                TransactionErrorState::ConfirmationUnknown
            }
            Self::ExecutionFailed { .. } => TransactionErrorState::ExecutionFailed,
        }
    }

    /// Returns the retained execution response when the transaction executed.
    pub fn response(&self) -> Option<&crate::sui::grpc::ExecuteTransactionResponse> {
        match self {
            Self::ConfirmationTimedOut { response, .. }
            | Self::ConfirmationFailed { response, .. }
            | Self::ConfirmationResponseInvalid { response, .. } => Some(response),
            Self::SubmissionRejected { .. }
            | Self::SubmissionUnknown { .. }
            | Self::ExecutionFailed { .. } => None,
        }
    }

    /// Returns the network status retained by
    /// [`TransactionError::SubmissionRejected`].
    pub fn submission_rejection(&self) -> Option<&tonic::Status> {
        match self {
            Self::SubmissionRejected { source, .. } => Some(source),
            Self::SubmissionUnknown { .. }
            | Self::ConfirmationTimedOut { .. }
            | Self::ConfirmationFailed { .. }
            | Self::ConfirmationResponseInvalid { .. }
            | Self::ExecutionFailed { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum NexusError {
    #[error(transparent)]
    Transaction(Box<TransactionError>),
    #[error("a private key is required for this operation")]
    MissingPrivateKey,
    #[error("Sui wallet error: {0}")]
    Wallet(anyhow::Error),
    #[error("Client configuration error: {0}")]
    Configuration(String),
    #[error("a gas source is already configured")]
    GasSourceAlreadyConfigured,
    #[error("Transaction building error: {0}")]
    TransactionBuilding(anyhow::Error),
    #[error("RPC error: {0}")]
    Rpc(anyhow::Error),
    #[error("Parsing error: {0}")]
    Parsing(anyhow::Error),
    #[error("Timeout error: {0}")]
    Timeout(anyhow::Error),
    #[error("Channel error: {0}")]
    Channel(anyhow::Error),
    #[error("Storage error: {0}")]
    Storage(anyhow::Error),
    #[error("Protocol validation error: {0}")]
    ProtocolValidation(anyhow::Error),
    #[error(
        "Protocol version {protocol_version} exceeds the newest version {maximum} supported by \
         this SDK"
    )]
    UnsupportedProtocolVersion { protocol_version: u64, maximum: u64 },
    #[error(
        "Protocol changed from version {client_version} to version {active_version} while the \
         operation was in progress"
    )]
    StaleProtocol {
        client_version: u64,
        active_version: u64,
    },
    #[error("Object '{object}' requires protocol version {required}, active version is {current}")]
    MigrationRequired {
        object: crate::sui::types::Address,
        current: u64,
        required: u64,
    },
    /// The selected [`Versioned`] payload schema has no matching SDK binding.
    ///
    /// [`Versioned`]: crate::move_bindings::sui_framework::versioned::Versioned
    #[error(
        "Object '{object}' uses state schema {actual}, but this SDK expects state schema {expected}"
    )]
    UnsupportedStateSchema {
        object: crate::sui::types::Address,
        actual: u64,
        expected: u64,
    },
}

impl From<TransactionError> for NexusError {
    fn from(error: TransactionError) -> Self {
        Self::Transaction(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{TransactionError, TransactionErrorState},
        crate::sui,
        std::time::Duration,
    };

    #[test]
    fn submission_unknown_retains_transaction_identity() {
        let digest = sui::types::Digest::new([7; 32]);
        let error = TransactionError::submission_unknown(
            digest,
            tonic::Status::unavailable("connection closed"),
        );

        assert_eq!(error.digest(), &digest);
        assert_eq!(error.state(), TransactionErrorState::SubmissionUnknown);
        assert!(error.response().is_none());
        assert!(!error.to_string().contains("retry"));
    }

    #[test]
    fn submission_rejection_is_known_and_retains_transaction_identity() {
        let digest = sui::types::Digest::new([7; 32]);
        let error = TransactionError::submission_rejected(
            digest,
            tonic::Status::invalid_argument("invalid gas reservation"),
        );

        assert_eq!(error.digest(), &digest);
        assert_eq!(error.state(), TransactionErrorState::SubmissionRejected);
        assert!(error.response().is_none());
        assert_eq!(
            error.submission_rejection().map(tonic::Status::message),
            Some("invalid gas reservation")
        );
        assert!(!error.to_string().contains("unknown"));
    }

    #[test]
    fn confirmation_unknown_retains_transaction_identity_and_response() {
        let digest = sui::types::Digest::new([7; 32]);
        let error = TransactionError::confirmation_timed_out(
            digest,
            Duration::from_secs(30),
            sui::grpc::ExecuteTransactionResponse::default(),
        );

        assert_eq!(error.digest(), &digest);
        assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
        assert!(error.response().is_some());
        assert!(!error.to_string().contains("retry"));
    }

    #[test]
    fn invalid_confirmation_response_retains_transaction_identity_and_response() {
        let digest = sui::types::Digest::new([7; 32]);
        let response = sui::grpc::ExecuteTransactionResponse::default();
        let error = TransactionError::confirmation_response_invalid(
            digest,
            response.clone(),
            "transaction is missing",
        );

        assert_eq!(error.digest(), &digest);
        assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
        assert_eq!(error.response(), Some(&response));
        assert!(!error.to_string().contains("retry"));
    }

    #[test]
    fn execution_failure_retains_transaction_identity() {
        let digest = sui::types::Digest::new([7; 32]);
        let error = TransactionError::execution_failed(
            digest,
            42,
            sui::types::ExecutionError::InsufficientGas,
            Some(3),
        );

        assert_eq!(error.digest(), &digest);
        assert_eq!(error.state(), TransactionErrorState::ExecutionFailed);
        assert!(error.response().is_none());
        assert!(!error.to_string().contains("retry"));
    }
}
