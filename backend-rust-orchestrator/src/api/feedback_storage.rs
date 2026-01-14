//! Feedback Storage Module
//!
//! Stores search relevance feedback in SQLite for Cross-Encoder fine-tuning.
//! This module provides a simple, append-only log of user feedback on search results.

use rusqlite::{Connection, params, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tracing::{info, warn, error};
use chrono::{DateTime, Utc};

/// Feedback entry representing a single user feedback on a search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub id: i64,
    pub query: String,
    pub document_id: String,
    pub is_relevant: bool,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
}

/// Feedback storage using SQLite
pub struct FeedbackStorage {
    conn: Arc<Mutex<Connection>>,
}

impl FeedbackStorage {
    /// Initialize feedback storage with SQLite database
    pub fn new(db_path: Option<PathBuf>) -> SqliteResult<Self> {
        let db_path = db_path.unwrap_or_else(|| {
            // Default to data directory
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("pagi-digital-twin");
            std::fs::create_dir_all(&data_dir).ok();
            data_dir.join("search_feedback.db")
        });

        info!(path = %db_path.display(), "Initializing feedback storage");

        let conn = Connection::open(&db_path)?;
        
        // Create table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS search_feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                document_id TEXT NOT NULL,
                is_relevant INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                session_id TEXT
            )",
            [],
        )?;

        // Create indexes for efficient querying
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feedback_query ON search_feedback(query)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feedback_timestamp ON search_feedback(timestamp)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_feedback_document ON search_feedback(document_id)",
            [],
        )?;

        info!("Feedback storage initialized successfully");

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Store a feedback entry
    pub fn store_feedback(
        &self,
        query: &str,
        document_id: &str,
        is_relevant: bool,
        session_id: Option<&str>,
    ) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let timestamp = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO search_feedback (query, document_id, is_relevant, timestamp, session_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![query, document_id, is_relevant as i32, timestamp, session_id],
        )?;

        let id = conn.last_insert_rowid();
        info!(
            id = id,
            query = %query,
            document_id = %document_id,
            is_relevant = is_relevant,
            "Feedback stored"
        );

        Ok(id)
    }

    /// Get all feedback entries
    pub fn get_all_feedback(&self) -> SqliteResult<Vec<FeedbackEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, query, document_id, is_relevant, timestamp, session_id
             FROM search_feedback
             ORDER BY timestamp DESC"
        )?;

        let feedback_iter = stmt.query_map([], |row| {
            Ok(FeedbackEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                document_id: row.get(2)?,
                is_relevant: row.get::<_, i32>(3)? != 0,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(4, "timestamp".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Utc),
                session_id: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for entry in feedback_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    /// Get feedback entries for a specific query
    pub fn get_feedback_for_query(&self, query: &str) -> SqliteResult<Vec<FeedbackEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, query, document_id, is_relevant, timestamp, session_id
             FROM search_feedback
             WHERE query = ?1
             ORDER BY timestamp DESC"
        )?;

        let feedback_iter = stmt.query_map(params![query], |row| {
            Ok(FeedbackEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                document_id: row.get(2)?,
                is_relevant: row.get::<_, i32>(3)? != 0,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map_err(|e| rusqlite::Error::InvalidColumnType(4, "timestamp".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Utc),
                session_id: row.get(5)?,
            })
        })?;

        let mut entries = Vec::new();
        for entry in feedback_iter {
            entries.push(entry?);
        }

        Ok(entries)
    }

    /// Get count of feedback entries
    pub fn get_feedback_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM search_feedback",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get feedback entries grouped by query for training
    pub fn get_feedback_grouped_by_query(&self) -> SqliteResult<HashMap<String, Vec<FeedbackEntry>>> {
        let all_feedback = self.get_all_feedback()?;
        let mut grouped: HashMap<String, Vec<FeedbackEntry>> = HashMap::new();

        for entry in all_feedback {
            grouped.entry(entry.query.clone()).or_insert_with(Vec::new).push(entry);
        }

        Ok(grouped)
    }
}
