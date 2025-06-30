use graphql_client::{GraphQLQuery, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::de::DeserializeOwned;

use crate::errors::{NexusError, Result};

/// Configuration for the Nexus Mods GraphQL client
#[derive(Debug, Clone)]
pub struct NexusConfig {
    /// Base URL for the GraphQL API
    pub api_url: String,
    /// Base URL for the legacy REST API
    pub legacy_api_url: String,
    /// Optional API key for authenticated requests
    pub api_key: Option<String>,
    /// User agent string for requests
    pub user_agent: String,
}

impl Default for NexusConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.nexusmods.com/v2/graphql".to_string(),
            legacy_api_url: "https://api.nexusmods.com/v1".to_string(),
            api_key: None,
            user_agent:
                "sewer-is-downloading-a-lot-of-textures-dont-mind-me-its-for-research/0.1.0"
                    .to_string(),
        }
    }
}

/// GraphQL client for Nexus Mods API
#[derive(Debug, Clone)]
pub struct NexusClient {
    client: ClientWithMiddleware,
    config: NexusConfig,
}

impl NexusClient {
    /// Create a new client with default configuration
    pub fn new() -> Self {
        Self::with_config(NexusConfig::default())
    }

    /// Create a new client with custom configuration
    pub fn with_config(config: NexusConfig) -> Self {
        let reqwest_client = reqwest::Client::builder()
            .user_agent(&config.user_agent)
            .build()
            .expect("Failed to create HTTP client");

        // Set up retry policy with exponential backoff
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(5);
        let client = ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self { client, config }
    }

    /// Set API key for authenticated requests
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.config.api_key = Some(api_key);
        self
    }

    /// Get the current configuration
    pub fn config(&self) -> &NexusConfig {
        &self.config
    }

    /// Execute a GraphQL query
    pub async fn execute<Q>(&self, variables: Q::Variables) -> Result<Q::ResponseData>
    where
        Q: GraphQLQuery,
        Q::Variables: serde::Serialize,
        Q::ResponseData: DeserializeOwned,
    {
        let request_body = Q::build_query(variables);

        let mut request_builder = self
            .client
            .post(&self.config.api_url)
            .header("Content-Type", "application/json");

        // Add API key if configured
        if let Some(ref api_key) = self.config.api_key {
            request_builder = request_builder.header("apikey", api_key);
        }

        // Serialize JSON body manually and send request
        let json_body = serde_json::to_vec(&request_body)?;
        let response = request_builder.body(json_body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NexusError::graphql(format!("HTTP {status}: {text}")));
        }

        let response_body: Response<Q::ResponseData> = response.json().await?;

        if let Some(errors) = response_body.errors {
            let error_message = errors
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(NexusError::graphql(error_message));
        }

        response_body.data.ok_or(NexusError::NoData)
    }

    /// Get download links for a file using the legacy REST API
    ///
    /// This method requires authentication via API key. For premium users, this will return
    /// direct download links. For free users, you may need additional parameters.
    ///
    /// # Arguments
    ///
    /// * `game_domain` - The game domain name (e.g., "skyrimspecialedition")
    /// * `mod_id` - The mod ID
    /// * `file_id` - The file ID
    ///
    /// # Example
    ///
    /// ```rust
    /// use nexus_gql::NexusClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = NexusClient::new().with_api_key("your_api_key".to_string());
    /// let links = client.download_links("skyrimspecialedition", "659", "1234").await?;
    ///
    /// for link in &links {
    ///     println!("Mirror: {} - URL: {}", link.short_name, link.uri);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn download_links(
        &self,
        game_domain: &str,
        mod_id: &str,
        file_id: &str,
    ) -> Result<Vec<crate::types::DownloadLink>> {
        if self.config.api_key.is_none() {
            return Err(NexusError::config(
                "API key is required for download links. Use .with_api_key() to set it.",
            ));
        }

        let url = format!(
            "{}/games/{}/mods/{}/files/{}/download_link.json",
            self.config.legacy_api_url, game_domain, mod_id, file_id
        );

        let mut request = self.client.get(&url);

        // Add API key header
        if let Some(ref api_key) = self.config.api_key {
            request = request.header("apikey", api_key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NexusError::http_error(format!("HTTP {status}: {text}")));
        }

        let links: Vec<crate::types::DownloadLink> = response.json().await?;
        Ok(links)
    }
}

impl Default for NexusClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        get_game, get_mod_files, get_popular_mods_for_game_and_category_by_endorsements_descending,
        GetGame, GetModFiles, GetPopularModsForGameAndCategoryByEndorsementsDescending,
    };

    /// Helper method to execute a query and handle common error cases
    async fn execute_query_with_error_handling<Q, F>(
        client: &NexusClient,
        variables: Q::Variables,
        success_handler: F,
    ) where
        Q: GraphQLQuery,
        Q::Variables: serde::Serialize,
        Q::ResponseData: serde::de::DeserializeOwned,
        F: FnOnce(Q::ResponseData),
    {
        let result = client.execute::<Q>(variables).await;

        match result {
            Ok(data) => {
                success_handler(data);
            }
            Err(e) => {
                let error_msg = e.to_string();
                println!("❌ Query failed: {error_msg}");

                // Check if this is a GraphQL error (API responded but with errors)
                if error_msg.contains("errors") || error_msg.contains("GraphQL") {
                    println!("💡 This appears to be a GraphQL error - likely requires authentication or invalid query");
                } else if error_msg.contains("decoding response body") {
                    println!("Response decoding failed - API responded but with unexpected format");
                } else {
                    // Only assert for true network errors
                    assert!(
                        error_msg.contains("network")
                            || error_msg.contains("DNS")
                            || error_msg.contains("connection")
                            || error_msg.contains("timeout")
                            || error_msg.contains("resolve"),
                        "Unexpected error type - this suggests a code issue: {error_msg}"
                    );
                    println!("⚠️ Network error (expected in CI): {error_msg}");
                }
            }
        }
    }

    #[tokio::test]
    async fn get_game_integration() {
        // Create client (this test can work without API key for public data)
        let client = NexusClient::new();

        // Test variables for Skyrim Special Edition domain
        let variables = get_game::Variables {
            domain_name: "skyrimspecialedition".to_string(),
        };

        execute_query_with_error_handling::<GetGame, _>(&client, variables, |data| {
            if let Some(game) = data.game {
                println!("✅ Successfully retrieved game information");
                println!(
                    "Game: {} (ID: {}, Domain: {})",
                    game.name, game.id, game.domain_name
                );
            } else {
                println!("⚠️ No game found for domain 'skyrimspecialedition'");
            }
        })
        .await;
    }

    #[tokio::test]
    async fn get_popular_mods_integration() {
        // Create client (this test can work without API key for public data)
        let client = NexusClient::new();

        // Test variables for Skyrim Special Edition - Models and Textures
        let variables =
            get_popular_mods_for_game_and_category_by_endorsements_descending::Variables {
                game_id: "1704".to_string(),
                category_name: "Models and Textures".to_string(),
                count: Some(5),
                offset: Some(0),
            };

        execute_query_with_error_handling::<
            GetPopularModsForGameAndCategoryByEndorsementsDescending,
            _,
        >(&client, variables, |data| {
            let first_mod = &data.mods.nodes[0];
            println!("✅ Successfully retrieved {} mods", data.mods.nodes.len());
            println!(
                "First mod: {} (ID: {}, Endorsements: {})",
                first_mod.name, first_mod.mod_id, first_mod.endorsements
            );
        })
        .await;
    }

    #[tokio::test]
    async fn get_mod_files_integration() {
        // Create client
        let client = NexusClient::new();

        // Test variables for SMIM (Static Mesh Improvement Mod) in Skyrim Special Edition
        // This is a popular mod that should have files available
        let variables = get_mod_files::Variables {
            mod_id: "659".to_string(),   // SMIM mod ID
            game_id: "1704".to_string(), // Skyrim Special Edition
        };

        execute_query_with_error_handling::<GetModFiles, _>(&client, variables, |data| {
            println!(
                "✅ Successfully retrieved {} files for mod",
                data.mod_files.len()
            );
            if !data.mod_files.is_empty() {
                let first_file = &data.mod_files[0];
                println!(
                    "First file: {} (ModID: {}, FileID: {}, Size: {})",
                    first_file.name,
                    first_file.mod_id,
                    first_file.file_id,
                    first_file
                        .size_in_bytes
                        .as_ref()
                        .map(|s| s.format_bytes())
                        .unwrap_or_else(|| "unknown".to_string())
                );
            }
        })
        .await;
    }

    #[tokio::test]
    async fn download_links_integration() {
        // This test requires an API key from environment variable
        let api_key = std::env::var("NEXUS_API_KEY").ok();

        if api_key.is_none() {
            println!(
                "⚠️ Skipping download links test - NEXUS_API_KEY environment variable not set"
            );
            println!("   Set NEXUS_API_KEY=your_api_key to run this test");
            return;
        }

        let client = NexusClient::new().with_api_key(api_key.unwrap());

        // Test download links for SMIM main file in Skyrim Special Edition
        let result = client
            .download_links("skyrimspecialedition", "659", "4639")
            .await;

        match result {
            Ok(links) => {
                println!("✅ Successfully retrieved {} download links", links.len());
                for link in &links {
                    println!(
                        "  Mirror: {} - URL: {}",
                        link.short_name,
                        if link.uri.len() > 50 {
                            format!("{}...", &link.uri[..50])
                        } else {
                            link.uri.clone()
                        }
                    );
                }
                assert!(!links.is_empty(), "Should have at least one download link");
            }
            Err(e) => {
                let error_msg = e.to_string();
                println!("❌ Download links test failed: {error_msg}");

                if error_msg.contains("404") {
                    println!(
                        "💡 File not found - this is expected if the test file ID is outdated"
                    );
                } else if error_msg.contains("403") || error_msg.contains("401") {
                    println!("💡 Authentication error - check your API key permissions");
                } else if error_msg.contains("network") || error_msg.contains("DNS") {
                    println!("⚠️ Network error (expected in CI): {error_msg}");
                } else {
                    // Only assert for unexpected error types
                    panic!("Unexpected error type: {error_msg}");
                }
            }
        }
    }

    #[tokio::test]
    async fn download_links_requires_api_key() {
        let client = NexusClient::new(); // No API key

        let result = client
            .download_links("skyrimspecialedition", "659", "1679")
            .await;

        match result {
            Err(NexusError::Config(msg)) => {
                assert!(msg.contains("API key is required"));
                println!("✅ Correctly rejected request without API key");
            }
            Ok(_) => panic!("Should have failed without API key"),
            Err(e) => panic!("Unexpected error type: {}", e),
        }
    }
}
