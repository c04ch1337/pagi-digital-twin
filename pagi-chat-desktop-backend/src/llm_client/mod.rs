use async_trait::async_trait;
use anyhow::{Result, bail};
use reqwest::Client;
use std::time::Duration;

use pagi_digital_twin_core::agent::{ExternalLLM, LLMCallInput, LLMCallOutput};

// --- Concrete Client Implementation (HTTP Endpoint) ---
pub struct AxumLLMClient {
    http_client: Client,
    endpoint_url: String,
}

impl AxumLLMClient {
    pub fn new(endpoint_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client for LLM");
        
        Self {
            http_client,
            endpoint_url,
        }
    }
}

#[async_trait]
impl ExternalLLM for AxumLLMClient {
    async fn call(&self, input: LLMCallInput) -> Result<LLMCallOutput> {
        tracing::debug!("Sending LLM call to: {} with schema: {}", self.endpoint_url, input.schema_name);

        let response = self.http_client.post(&self.endpoint_url)
            .json(&input)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            bail!("LLM API call failed with status: {}. Body: {}", status, text);
        }

        let raw_response = response.text().await?;
        
        // Find and extract the JSON block based on the expected schema (e.g., parsing a markdown block)
        // NOTE: The core crate will need robust JSON extraction logic, but we model the output here.
        let json_body = match serde_json::from_str(&raw_response) {
            Ok(v) => v,
            Err(_) => {
                // If it's not pure JSON, attempt to find a JSON code block in the response text
                if let Some(start) = raw_response.find("```json") {
                    if let Some(end) = raw_response[start + 7..].find("```") {
                        let json_str = &raw_response[start + 7..start + 7 + end];
                        serde_json::from_str(json_str.trim()).unwrap_or_default()
                    } else {
                        bail!("Failed to parse JSON body from raw response.")
                    }
                } else {
                    // Fallback for debugging
                    bail!("Response was neither pure JSON nor contained a '```json' block.")
                }
            }
        };

        Ok(LLMCallOutput {
            raw_response,
            json_body,
        })
    }
}
