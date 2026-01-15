pub mod system;
pub mod monitor;
pub mod doctor;
pub mod nmap_models;
pub mod network_scanner;
pub mod git;
pub mod github_tool_finder;
pub mod safe_installer;
pub mod audit_archiver;
pub mod playbook_store;

pub use system::{get_logs, manage_service, read_file, run_command, systemctl, write_file};
pub use monitor::{get_system_snapshot, SystemSnapshot};
pub use doctor::agi_doctor;
pub use git::GitOperations;
pub use github_tool_finder::{find_github_tool, propose_tool_installation, ToolDiscoveryResult};
pub use audit_archiver::{archive_audit_report, search_audit_history, analyze_audit_trends, AuditReport, HistoricalAuditReport, TrendAnalysis};
pub use playbook_store::{Playbook, PlaybookSearchResult, save_playbook, search_playbooks_by_tool, search_playbooks_by_query, get_all_playbooks, update_playbook_stats, ensure_playbook_collection};