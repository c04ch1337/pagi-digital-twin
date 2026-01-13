use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

/// Microsoft Graph API client for email and Teams monitoring
pub struct EmailTeamsMonitor {
    http_client: reqwest::Client,
    access_token: Arc<RwLock<Option<String>>>,
    refresh_token: Arc<RwLock<Option<String>>>,
    client_id: String,
    client_secret: String,
    tenant_id: String,
    user_email: String,
    user_name: String,
    redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: EmailAddress,
    pub to_recipients: Vec<EmailAddress>,
    pub cc_recipients: Vec<EmailAddress>,
    pub body: EmailBody,
    pub received_date_time: DateTime<Utc>,
    pub is_read: bool,
    pub importance: String,
    pub has_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailBody {
    pub content_type: String, // "text" or "html"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsMessage {
    pub id: String,
    pub chat_id: String,
    pub channel_id: Option<String>,
    pub from: TeamsUser,
    pub body: TeamsMessageBody,
    pub created_date_time: DateTime<Utc>,
    pub message_type: String, // "message", "systemMessage", etc.
    pub mentions: Vec<TeamsMention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsUser {
    pub id: String,
    pub display_name: String,
    pub user_principal_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsMessageBody {
    pub content_type: String, // "text" or "html"
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamsMention {
    pub id: u32,
    pub mention_text: String,
    pub mentioned: TeamsUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTrend {
    pub period: String, // "day", "week", "month"
    pub total_emails: u32,
    pub unread_count: u32,
    pub urgent_count: u32,
    pub from_top_senders: Vec<SenderStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderStats {
    pub email: String,
    pub name: Option<String>,
    pub count: u32,
}

impl EmailTeamsMonitor {
    pub fn new(
        client_id: String,
        client_secret: String,
        tenant_id: String,
        user_email: String,
        user_name: String,
        redirect_uri: String,
    ) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(None)),
            refresh_token: Arc::new(RwLock::new(None)),
            client_id,
            client_secret,
            tenant_id,
            user_email,
            user_name,
            redirect_uri,
        }
    }

    /// Set OAuth access token (called after OAuth flow completes)
    pub async fn set_access_token(&self, access_token: String, refresh_token: Option<String>) {
        *self.access_token.write().await = Some(access_token);
        if let Some(refresh) = refresh_token {
            *self.refresh_token.write().await = Some(refresh);
        }
    }

    /// Exchange authorization code for access token
    pub async fn exchange_code_for_token(&self, code: &str) -> Result<(String, Option<String>), String> {
        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );

        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to exchange code for token: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Token exchange failed ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("Missing access_token in response")?
            .to_string();

        let refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Store tokens
        self.set_access_token(access_token.clone(), refresh_token.clone()).await;

        Ok((access_token, refresh_token))
    }

    /// Refresh access token using refresh token
    async fn refresh_access_token(&self) -> Result<String, String> {
        let refresh_token_guard = self.refresh_token.read().await;
        let refresh_token = refresh_token_guard.clone();
        drop(refresh_token_guard);

        let refresh_token = refresh_token.ok_or("No refresh token available")?;

        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        );

        let params = [
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
            ("refresh_token", &refresh_token),
            ("grant_type", "refresh_token"),
            ("scope", "Mail.Read Mail.Send Chat.Read Chat.Send User.Read offline_access"),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to refresh token: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Token refresh failed ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        let access_token = json
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("Missing access_token in refresh response")?
            .to_string();

        let new_refresh_token = json
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Update tokens
        self.set_access_token(access_token.clone(), new_refresh_token).await;

        Ok(access_token)
    }

    /// Check if token is expired by attempting to decode it (simple check)
    /// Note: This is a basic check. For production, use proper JWT decoding
    fn is_token_expired(&self, token: &str) -> bool {
        // Basic check: if token is too short, consider it invalid
        if token.len() < 50 {
            return true;
        }
        
        // For a more accurate check, we could decode the JWT and check the exp claim
        // For now, we'll rely on API errors to indicate expiration
        false
    }

    /// Get current access token, refreshing if needed
    async fn get_valid_token(&self) -> Result<String, String> {
        let token_guard = self.access_token.read().await;
        let token = token_guard.clone();
        drop(token_guard);

        if let Some(token) = token {
            // Try to use the token - if it fails with 401, refresh it
            // For now, we'll attempt refresh if we have a refresh token
            // In a production system, you'd decode the JWT to check expiration
            return Ok(token);
        }
        
        // No token, try to refresh
        if self.refresh_token.read().await.is_some() {
            return self.refresh_access_token().await;
        }
        
        Err("No access token available. Please complete OAuth authentication.".to_string())
    }

    /// Get valid token with automatic retry on 401
    async fn get_valid_token_with_retry(&self) -> Result<String, String> {
        let token = self.get_valid_token().await?;
        
        // We'll validate the token by making a test call if needed
        // For now, just return it - actual validation happens on API calls
        Ok(token)
    }

    /// Make API request with automatic token refresh on 401
    /// This is a helper that wraps API calls to automatically refresh tokens on 401 errors
    async fn make_api_request_with_refresh(
        &self,
        url: &str,
        method: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<reqwest::Response, String> {
        let token = self.get_valid_token_with_retry().await?;
        
        let mut request = match method {
            "GET" => self.http_client.get(url),
            "POST" => self.http_client.post(url),
            _ => return Err("Unsupported HTTP method".to_string()),
        };
        
        request = request
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json");
        
        if let Some(body_json) = body {
            request = request.json(body_json);
        }
        
        let response = request.send().await
            .map_err(|e| format!("API request failed: {}", e))?;

        // If we get 401, try refreshing the token and retry once
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            warn!("Received 401, attempting token refresh...");
            let new_token = self.refresh_access_token().await?;
            
            // Retry the request with new token
            let mut retry_request = match method {
                "GET" => self.http_client.get(url),
                "POST" => self.http_client.post(url),
                _ => return Err("Unsupported HTTP method".to_string()),
            };
            
            retry_request = retry_request
                .header("Authorization", format!("Bearer {}", new_token))
                .header("Content-Type", "application/json");
            
            if let Some(body_json) = body {
                retry_request = retry_request.json(body_json);
            }
            
            let retry_response = retry_request.send().await
                .map_err(|e| format!("Retry request failed: {}", e))?;
            
            return Ok(retry_response);
        }

        Ok(response)
    }

    /// Check for new emails that address the user specifically
    pub async fn check_new_emails(&self, filter_unread: bool) -> Result<Vec<EmailMessage>, String> {
        let token = self.get_valid_token_with_retry().await?;
        
        let filter = if filter_unread {
            "isRead eq false"
        } else {
            ""
        };

        let url = format!(
            "https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages?$filter={}&$orderby=receivedDateTime desc&$top=50",
            filter
        );

        let response = self.make_api_request_with_refresh(&url, "GET", None).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Graph API error ({}): {}", status, body));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse email response: {}", e))?;

        let mut emails = Vec::new();
        if let Some(value_array) = json.get("value").and_then(|v| v.as_array()) {
            for email_json in value_array {
                if let Ok(email) = self.parse_email_message(email_json) {
                    // Filter for emails that address the user
                    if self.is_addressed_to_user(&email) {
                        emails.push(email);
                    }
                }
            }
        }

        Ok(emails)
    }

    /// Parse a Graph API email JSON into EmailMessage
    fn parse_email_message(&self, json: &serde_json::Value) -> Result<EmailMessage, String> {
        let id = json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing id")?
            .to_string();

        let subject = json
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("(No Subject)")
            .to_string();

        let from_json = json.get("from").ok_or("Missing from")?;
        let from = EmailAddress {
            name: from_json.get("emailAddress").and_then(|e| e.get("name")).and_then(|n| n.as_str()).map(|s| s.to_string()),
            address: from_json
                .get("emailAddress")
                .and_then(|e| e.get("address"))
                .and_then(|a| a.as_str())
                .ok_or("Missing from address")?
                .to_string(),
        };

        let to_recipients = json
            .get("toRecipients")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        r.get("emailAddress").map(|e| EmailAddress {
                            name: e.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()),
                            address: e.get("address").and_then(|a| a.as_str()).map(|s| s.to_string()).unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cc_recipients = json
            .get("ccRecipients")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        r.get("emailAddress").map(|e| EmailAddress {
                            name: e.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()),
                            address: e.get("address").and_then(|a| a.as_str()).map(|s| s.to_string()).unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let body_json = json.get("body").ok_or("Missing body")?;
        let body = EmailBody {
            content_type: body_json.get("contentType").and_then(|v| v.as_str()).unwrap_or("text").to_string(),
            content: body_json.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        let received_date_time = json
            .get("receivedDateTime")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let is_read = json.get("isRead").and_then(|v| v.as_bool()).unwrap_or(false);
        let importance = json.get("importance").and_then(|v| v.as_str()).unwrap_or("normal").to_string();
        let has_attachments = json.get("hasAttachments").and_then(|v| v.as_bool()).unwrap_or(false);

        Ok(EmailMessage {
            id,
            subject,
            from,
            to_recipients,
            cc_recipients,
            body,
            received_date_time,
            is_read,
            importance,
            has_attachments,
        })
    }

    /// Detect if email is addressed to the user
    fn is_addressed_to_user(&self, email: &EmailMessage) -> bool {
        let user_email_lower = self.user_email.to_lowercase();
        let user_name_lower = self.user_name.to_lowercase();

        // Check To recipients
        for recipient in &email.to_recipients {
            if recipient.address.to_lowercase() == user_email_lower {
                return true;
            }
            if let Some(ref name) = recipient.name {
                if name.to_lowercase().contains(&user_name_lower) {
                    return true;
                }
            }
        }

        // Check CC recipients
        for recipient in &email.cc_recipients {
            if recipient.address.to_lowercase() == user_email_lower {
                return true;
            }
        }

        // Check body for name mentions (simple heuristic)
        let body_lower = email.body.content.to_lowercase();
        if body_lower.contains(&user_name_lower) {
            // Check for greeting patterns
            let patterns = [
                format!("dear {}", user_name_lower),
                format!("hi {}", user_name_lower),
                format!("hello {}", user_name_lower),
                format!("hey {}", user_name_lower),
                format!("{}:", user_name_lower),
            ];
            for pattern in &patterns {
                if body_lower.contains(pattern) {
                    return true;
                }
            }
        }

        false
    }

    /// Check for new Teams messages (mentions and direct messages)
    pub async fn check_teams_messages(&self) -> Result<Vec<TeamsMessage>, String> {
        let token = self.get_valid_token_with_retry().await?;

        // Get chats (1-on-1 and group chats)
        let chats_url = "https://graph.microsoft.com/v1.0/me/chats?$top=50";
        let response = self
            .http_client
            .get(chats_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Teams chats: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Graph API error ({}): {}", status, body));
        }

        let chats_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse chats response: {}", e))?;

        let mut all_messages = Vec::new();

        // For each chat, get recent messages
        if let Some(chats_array) = chats_json.get("value").and_then(|v| v.as_array()) {
            for chat_json in chats_array {
                if let Some(chat_id) = chat_json.get("id").and_then(|v| v.as_str()) {
                    let messages_url = format!(
                        "https://graph.microsoft.com/v1.0/me/chats/{}/messages?$top=20&$orderby=createdDateTime desc",
                        chat_id
                    );

                    let msg_response = self
                        .http_client
                        .get(&messages_url)
                        .header("Authorization", format!("Bearer {}", token))
                        .send()
                        .await;

                    if let Ok(msg_response) = msg_response {
                        if msg_response.status().is_success() {
                            if let Ok(msg_json) = msg_response.json::<serde_json::Value>().await {
                                if let Some(messages_array) = msg_json.get("value").and_then(|v| v.as_array()) {
                                    for msg_json in messages_array {
                                        if let Ok(message) = self.parse_teams_message(msg_json, chat_id) {
                                            // Check if user is mentioned or it's a direct message
                                            if self.is_teams_message_relevant(&message) {
                                                all_messages.push(message);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(all_messages)
    }

    /// Parse a Graph API Teams message JSON
    fn parse_teams_message(&self, json: &serde_json::Value, chat_id: &str) -> Result<TeamsMessage, String> {
        let id = json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or("Missing id")?
            .to_string();

        let from_json = json.get("from").ok_or("Missing from")?;
        let from = TeamsUser {
            id: from_json.get("user").and_then(|u| u.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
            display_name: from_json.get("user").and_then(|u| u.get("displayName")).and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
            user_principal_name: from_json.get("user").and_then(|u| u.get("userPrincipalName")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        let body_json = json.get("body").ok_or("Missing body")?;
        let body = TeamsMessageBody {
            content_type: body_json.get("contentType").and_then(|v| v.as_str()).unwrap_or("text").to_string(),
            content: body_json.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        };

        let created_date_time = json
            .get("createdDateTime")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let message_type = json.get("messageType").and_then(|v| v.as_str()).unwrap_or("message").to_string();

        let mentions = json
            .get("mentions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        Some(TeamsMention {
                            id: m.get("id")?.as_u64()? as u32,
                            mention_text: m.get("mentionText")?.as_str()?.to_string(),
                            mentioned: TeamsUser {
                                id: m.get("mentioned")?.get("id")?.as_str()?.to_string(),
                                display_name: m.get("mentioned")?.get("displayName")?.as_str()?.to_string(),
                                user_principal_name: m.get("mentioned")?.get("userPrincipalName")?.as_str()?.to_string(),
                            },
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(TeamsMessage {
            id,
            chat_id: chat_id.to_string(),
            channel_id: json.get("channelIdentity").and_then(|c| c.get("channelId")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            from,
            body,
            created_date_time,
            message_type,
            mentions,
        })
    }

    /// Check if Teams message is relevant (user is mentioned or it's a direct message)
    fn is_teams_message_relevant(&self, message: &TeamsMessage) -> bool {
        // Check if user is mentioned
        let user_email_lower = self.user_email.to_lowercase();
        let user_name_lower = self.user_name.to_lowercase();

        for mention in &message.mentions {
            if mention.mentioned.user_principal_name.to_lowercase() == user_email_lower {
                return true;
            }
            if mention.mentioned.display_name.to_lowercase().contains(&user_name_lower) {
                return true;
            }
        }

        // For 1-on-1 chats, all messages are relevant
        // (We can't easily distinguish 1-on-1 from group chats without additional API calls)
        // For now, include all messages with mentions or in small chats
        true // Simplified: include all for now
    }

    /// Send email reply
    pub async fn send_email_reply(
        &self,
        original_email_id: String,
        reply_body: String,
    ) -> Result<String, String> {
        let token = self.get_valid_token_with_retry().await?;

        // First, get the original email to extract subject and recipients
        let get_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}", original_email_id);
        let response = self
            .http_client
            .get(&get_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch original email: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Failed to fetch original email: {}", response.status()));
        }

        let email_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse email: {}", e))?;

        let subject = email_json
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("Re:")
            .to_string();
        let reply_subject = if subject.starts_with("Re:") {
            subject
        } else {
            format!("Re: {}", subject)
        };

        // Build reply payload
        let reply_payload = json!({
            "message": {
                "subject": reply_subject,
                "body": {
                    "contentType": "HTML",
                    "content": format!("<div>{}</div>", reply_body.replace("\n", "<br>"))
                },
                "toRecipients": email_json.get("from").and_then(|f| {
                    f.get("emailAddress").map(|e| vec![json!({
                        "emailAddress": {
                            "name": e.get("name"),
                            "address": e.get("address")
                        }
                    })])
                }).unwrap_or_default()
            }
        });

        let send_url = format!("https://graph.microsoft.com/v1.0/me/messages/{}/reply", original_email_id);
        let send_response = self
            .http_client
            .post(&send_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&reply_payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send reply: {}", e))?;

        if !send_response.status().is_success() {
            let status = send_response.status();
            let body = send_response.text().await.unwrap_or_default();
            return Err(format!("Failed to send email reply ({}): {}", status, body));
        }

        Ok("Email reply sent successfully".to_string())
    }

    /// Send Teams message
    pub async fn send_teams_message(
        &self,
        chat_id: String,
        message_content: String,
    ) -> Result<String, String> {
        let token = self.get_valid_token_with_retry().await?;

        let payload = json!({
            "body": {
                "contentType": "html",
                "content": message_content
            }
        });

        let url = format!("https://graph.microsoft.com/v1.0/me/chats/{}/messages", chat_id);
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to send Teams message: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Failed to send Teams message ({}): {}", status, body));
        }

        Ok("Teams message sent successfully".to_string())
    }

    /// Get email trends/statistics
    pub async fn get_email_trends(&self, period: &str) -> Result<EmailTrend, String> {
        let token = self.get_valid_token_with_retry().await?;

        // Calculate date filter based on period
        let days = match period {
            "day" => 1,
            "week" => 7,
            "month" => 30,
            _ => 7,
        };
        let since = Utc::now() - chrono::Duration::days(days);
        let filter = format!("receivedDateTime ge {}", since.format("%Y-%m-%dT%H:%M:%SZ"));

        let url = format!(
            "https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages?$filter={}&$top=1000",
            filter
        );

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to fetch email trends: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Failed to fetch email trends: {}", response.status()));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse trends: {}", e))?;

        let mut total_emails = 0;
        let mut unread_count = 0;
        let mut urgent_count = 0;
        let mut sender_counts: HashMap<String, SenderStats> = HashMap::new();

        if let Some(value_array) = json.get("value").and_then(|v| v.as_array()) {
            for email_json in value_array {
                total_emails += 1;
                if email_json.get("isRead").and_then(|v| v.as_bool()).unwrap_or(false) == false {
                    unread_count += 1;
                }
                if email_json.get("importance").and_then(|v| v.as_str()).unwrap_or("normal") == "high" {
                    urgent_count += 1;
                }

                // Count by sender
                if let Some(from) = email_json.get("from").and_then(|f| f.get("emailAddress")) {
                    let address = from.get("address").and_then(|a| a.as_str()).unwrap_or("unknown").to_string();
                    let name = from.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());
                    let entry = sender_counts.entry(address.clone()).or_insert_with(|| SenderStats {
                        email: address,
                        name,
                        count: 0,
                    });
                    entry.count += 1;
                }
            }
        }

        let mut top_senders: Vec<SenderStats> = sender_counts.into_values().collect();
        top_senders.sort_by(|a, b| b.count.cmp(&a.count));
        top_senders.truncate(10);

        Ok(EmailTrend {
            period: period.to_string(),
            total_emails,
            unread_count,
            urgent_count,
            from_top_senders: top_senders,
        })
    }
}
