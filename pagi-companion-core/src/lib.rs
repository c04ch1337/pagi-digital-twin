//! pagi-companion-core (stub)
//!
//! This repository currently includes a minimal placeholder core crate so that
//! [`pagi-chat-desktop-backend`](../pagi-chat-desktop-backend/Cargo.toml:1) can compile.
//!
//! Replace/expand this crate with the real "source of truth" agent logic as it
//! becomes available.

pub mod companion {
    pub mod agent {
        use anyhow::Result;
        use tracing::info;

        #[derive(Debug, Clone)]
        pub struct CompanionAgent {
            user_id: String,
        }

        impl CompanionAgent {
            pub async fn new(user_id: &str) -> Result<Self> {
                info!(user_id = user_id, "CompanionAgent initialized (core stub crate)");
                Ok(Self {
                    user_id: user_id.to_string(),
                })
            }

            pub async fn process_user_input(&self, input: String) -> Result<String> {
                info!(user_id = %self.user_id, input = %input, "Processing input (core stub crate)");
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                Ok(format!("ACK(core): {}", input))
            }
        }
    }
}

