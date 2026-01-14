use regex::Regex;
use tracing::{info, warn};

/// Privacy filter that scrubs sensitive information from content before git commits
pub struct PrivacyFilter {
    ipv4_regex: Regex,
    ipv6_regex: Regex,
    username_regex: Regex,
    mac_address_regex: Regex,
    api_key_regex: Regex,
    secret_regex: Regex,
    token_regex: Regex,
}

impl PrivacyFilter {
    /// Create a new PrivacyFilter with compiled regex patterns
    pub fn new() -> Self {
        Self {
            // IPv4: matches patterns like 192.168.1.1, 10.0.0.1, etc.
            ipv4_regex: Regex::new(r"\b\d{1,3}(\.\d{1,3}){3}\b").expect("Invalid IPv4 regex"),
            
            // IPv6: matches patterns like 2001:0db8:85a3:0000:0000:8a2e:0370:7334 or ::1
            ipv6_regex: Regex::new(r"\b([0-9a-fA-F]{0,4}:){2,7}[0-9a-fA-F]{0,4}\b|::1\b").expect("Invalid IPv6 regex"),
            
            // Usernames: matches patterns after /Users/ or /home/
            // Examples: /Users/john, /home/jane, C:\Users\bob
            username_regex: Regex::new(r"(?:/Users/|/home/|C:\\Users\\)([A-Za-z0-9_-]+)").expect("Invalid username regex"),
            
            // MAC Addresses: matches patterns like 00:1B:44:11:3A:B7 or 00-1B-44-11-3A-B7
            mac_address_regex: Regex::new(r"([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})").expect("Invalid MAC address regex"),
            
            // API Keys: matches high-entropy strings following 'API_KEY', 'api_key', etc.
            // Pattern: API_KEY= followed by alphanumeric string of 20+ chars
            api_key_regex: Regex::new(r"(?i)(?:API[_-]?KEY|APIKEY)[\s:=]+([A-Za-z0-9_\-]{20,})").expect("Invalid API key regex"),
            
            // Secrets: matches patterns like SECRET=, secret=, etc. followed by high-entropy strings
            secret_regex: Regex::new(r"(?i)(?:SECRET|PASSWORD|PASSWD)[\s:=]+([A-Za-z0-9_\-+/=]{16,})").expect("Invalid secret regex"),
            
            // Tokens: matches patterns like TOKEN=, access_token=, etc.
            token_regex: Regex::new(r"(?i)(?:TOKEN|ACCESS[_-]?TOKEN|BEARER[_-]?TOKEN)[\s:=]+([A-Za-z0-9_\-+/=]{20,})").expect("Invalid token regex"),
        }
    }

    /// Scrub sensitive information from playbook content
    /// Replaces matches with <REDACTED> placeholder
    pub fn scrub_playbook(&self, content: String) -> String {
        let mut scrubbed = content;

        // Count replacements for logging
        let mut replacement_count = 0;

        // Scrub IPv4 addresses
        let ipv4_count = self.ipv4_regex.find_iter(&scrubbed).count();
        if ipv4_count > 0 {
            scrubbed = self.ipv4_regex.replace_all(&scrubbed, "<REDACTED_IP>").to_string();
            replacement_count += ipv4_count;
        }

        // Scrub IPv6 addresses
        let ipv6_count = self.ipv6_regex.find_iter(&scrubbed).count();
        if ipv6_count > 0 {
            scrubbed = self.ipv6_regex.replace_all(&scrubbed, "<REDACTED_IPV6>").to_string();
            replacement_count += ipv6_count;
        }

        // Scrub usernames from paths
        let username_count = self.username_regex.find_iter(&scrubbed).count();
        if username_count > 0 {
            scrubbed = self.username_regex.replace_all(&scrubbed, |caps: &regex::Captures| {
                if let Some(matched) = caps.get(0) {
                    // Replace the entire path with placeholder
                    matched.as_str().replace(&caps[1], "<USERNAME>")
                } else {
                    "<USERNAME>".to_string()
                }
            }).to_string();
            replacement_count += username_count;
        }

        // Scrub MAC addresses
        let mac_count = self.mac_address_regex.find_iter(&scrubbed).count();
        if mac_count > 0 {
            scrubbed = self.mac_address_regex.replace_all(&scrubbed, "<REDACTED_MAC>").to_string();
            replacement_count += mac_count;
        }

        // Scrub API keys
        let api_key_count = self.api_key_regex.find_iter(&scrubbed).count();
        if api_key_count > 0 {
            scrubbed = self.api_key_regex.replace_all(&scrubbed, |caps: &regex::Captures| {
                if let Some(full_match) = caps.get(0) {
                    // Replace the entire match, keeping the key name but redacting the value
                    if let Some(key_match) = caps.get(1) {
                        full_match.as_str().replace(key_match.as_str(), "<REDACTED>")
                    } else {
                        "<REDACTED_API_KEY>".to_string()
                    }
                } else {
                    "<REDACTED_API_KEY>".to_string()
                }
            }).to_string();
            replacement_count += api_key_count;
        }

        // Scrub secrets
        let secret_count = self.secret_regex.find_iter(&scrubbed).count();
        if secret_count > 0 {
            scrubbed = self.secret_regex.replace_all(&scrubbed, |caps: &regex::Captures| {
                if let Some(full_match) = caps.get(0) {
                    if let Some(secret_match) = caps.get(1) {
                        full_match.as_str().replace(secret_match.as_str(), "<REDACTED>")
                    } else {
                        "<REDACTED_SECRET>".to_string()
                    }
                } else {
                    "<REDACTED_SECRET>".to_string()
                }
            }).to_string();
            replacement_count += secret_count;
        }

        // Scrub tokens
        let token_count = self.token_regex.find_iter(&scrubbed).count();
        if token_count > 0 {
            scrubbed = self.token_regex.replace_all(&scrubbed, |caps: &regex::Captures| {
                if let Some(full_match) = caps.get(0) {
                    if let Some(token_match) = caps.get(1) {
                        full_match.as_str().replace(token_match.as_str(), "<REDACTED>")
                    } else {
                        "<REDACTED_TOKEN>".to_string()
                    }
                } else {
                    "<REDACTED_TOKEN>".to_string()
                }
            }).to_string();
            replacement_count += token_count;
        }

        if replacement_count > 0 {
            info!(
                replacements = replacement_count,
                "Privacy filter scrubbed sensitive information"
            );
        }

        scrubbed
    }

    /// Scrub a file path (useful for file names that might contain sensitive info)
    pub fn scrub_path(&self, path: &str) -> String {
        let mut scrubbed = path.to_string();
        
        // Remove usernames from paths
        scrubbed = self.username_regex.replace_all(&scrubbed, |caps: &regex::Captures| {
            if let Some(matched) = caps.get(0) {
                matched.as_str().replace(&caps[1], "<USERNAME>")
            } else {
                "<USERNAME>".to_string()
            }
        }).to_string();
        
        scrubbed
    }
}

impl Default for PrivacyFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrub_ipv4() {
        let filter = PrivacyFilter::new();
        let content = "Connecting to 192.168.1.100 on port 8080".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED_IP>"));
        assert!(!scrubbed.contains("192.168.1.100"));
    }

    #[test]
    fn test_scrub_ipv6() {
        let filter = PrivacyFilter::new();
        let content = "IPv6 address: 2001:0db8:85a3:0000:0000:8a2e:0370:7334".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED_IPV6>"));
    }

    #[test]
    fn test_scrub_username_from_path() {
        let filter = PrivacyFilter::new();
        let content = "File located at /Users/c04ch1337/pagi-agent-repo/secrets.env".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<USERNAME>"));
        assert!(!scrubbed.contains("c04ch1337"));
        // Verify the path structure is preserved but username is redacted
        assert!(scrubbed.contains("/Users/"));
    }

    #[test]
    fn test_scrub_username_from_home_path() {
        let filter = PrivacyFilter::new();
        let content = "Config file: /home/john/.config/app.conf".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<USERNAME>"));
        assert!(!scrubbed.contains("john"));
    }

    #[test]
    fn test_scrub_username_from_windows_path() {
        let filter = PrivacyFilter::new();
        let content = "Path: C:\\Users\\jane\\Documents\\file.txt".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<USERNAME>"));
        assert!(!scrubbed.contains("jane"));
    }

    #[test]
    fn test_scrub_mac_address() {
        let filter = PrivacyFilter::new();
        let content = "MAC: 00:1B:44:11:3A:B7".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED_MAC>"));
        assert!(!scrubbed.contains("00:1B:44:11:3A:B7"));
    }

    #[test]
    fn test_scrub_mac_address_with_dashes() {
        let filter = PrivacyFilter::new();
        let content = "MAC: 00-1B-44-11-3A-B7".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED_MAC>"));
    }

    #[test]
    fn test_scrub_api_key() {
        let filter = PrivacyFilter::new();
        // Use a clearly fake test key that won't trigger secret scanners
        let content = "API_KEY=sk_test_FAKE_KEY_FOR_TESTING_ONLY_NOT_A_REAL_SECRET_12345".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED>"));
        assert!(!scrubbed.contains("sk_test_FAKE_KEY_FOR_TESTING_ONLY_NOT_A_REAL_SECRET_12345"));
        // Should preserve API_KEY= prefix
        assert!(scrubbed.contains("API_KEY"));
    }

    #[test]
    fn test_scrub_secret() {
        let filter = PrivacyFilter::new();
        let content = "SECRET=MySuperSecretPassword123!@#".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED>"));
        assert!(!scrubbed.contains("MySuperSecretPassword123!@#"));
    }

    #[test]
    fn test_scrub_token() {
        let filter = PrivacyFilter::new();
        let content = "ACCESS_TOKEN=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ".to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED>"));
        assert!(!scrubbed.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_scrub_multiple_patterns() {
        let filter = PrivacyFilter::new();
        let content = r#"
        Connecting to 192.168.1.1
        User: /Users/c04ch1337/pagi-agent-repo/secrets.env
        MAC: 00:1B:44:11:3A:B7
        API_KEY=sk_test_1234567890abcdef
        "#.to_string();
        let scrubbed = filter.scrub_playbook(content);
        assert!(scrubbed.contains("<REDACTED_IP>"));
        assert!(scrubbed.contains("<USERNAME>"));
        assert!(scrubbed.contains("<REDACTED_MAC>"));
        assert!(scrubbed.contains("<REDACTED>"));
        assert!(!scrubbed.contains("192.168.1.1"));
        assert!(!scrubbed.contains("c04ch1337"));
        assert!(!scrubbed.contains("00:1B:44:11:3A:B7"));
        assert!(!scrubbed.contains("sk_test_1234567890abcdef"));
    }

    #[test]
    fn test_scrub_path_function() {
        let filter = PrivacyFilter::new();
        let path = "/Users/c04ch1337/pagi-agent-repo/secrets.env";
        let scrubbed = filter.scrub_path(path);
        assert!(scrubbed.contains("<USERNAME>"));
        assert!(!scrubbed.contains("c04ch1337"));
    }

    #[test]
    fn test_no_false_positives() {
        let filter = PrivacyFilter::new();
        // Content that should NOT be scrubbed
        let content = "Version 1.2.3.4 is available. User count: 12345".to_string();
        let scrubbed = filter.scrub_playbook(content);
        // Should not contain redaction tags for version numbers or user counts
        assert!(!scrubbed.contains("<REDACTED"));
        assert!(scrubbed.contains("1.2.3.4"));
        assert!(scrubbed.contains("12345"));
    }
}
