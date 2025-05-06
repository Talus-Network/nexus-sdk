use nexus_sdk::walrus::WalrusClient;

/// Configuration for Walrus client
#[derive(Default)]
pub struct WalrusConfig {
    /// The walrus publisher URL
    pub publisher_url: Option<String>,
    /// The URL of the aggregator
    pub aggregator_url: Option<String>,
}

impl WalrusConfig {
    /// Create a new WalrusConfig with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the publisher URL
    pub fn with_publisher_url(mut self, url: Option<String>) -> Self {
        self.publisher_url = url;
        self
    }

    /// Set the aggregator URL
    pub fn with_aggregator_url(mut self, url: Option<String>) -> Self {
        self.aggregator_url = url;
        self
    }

    /// Build a WalrusClient with the configured settings
    pub fn build(self) -> WalrusClient {
        let mut client_builder = WalrusClient::builder();

        if let Some(publisher_url) = self.publisher_url {
            client_builder = client_builder.with_publisher_url(&publisher_url);
        }

        if let Some(aggregator_url) = self.aggregator_url {
            client_builder = client_builder.with_aggregator_url(&aggregator_url);
        }

        client_builder.build()
    }
}
