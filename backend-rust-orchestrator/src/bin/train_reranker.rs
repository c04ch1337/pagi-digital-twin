//! Cross-Encoder Fine-Tuning Pipeline
//!
//! This utility script processes search feedback data and prepares it for
//! Cross-Encoder model fine-tuning. It:
//! 1. Loads feedback entries from SQLite database
//! 2. Groups entries into positive/negative pairs per query
//! 3. Exports data in JSONL format compatible with HuggingFace SentenceTransformers
//! 4. Optionally triggers local training run

use std::collections::HashMap;
use std::path::PathBuf;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use anyhow::{Result, Context};
use rusqlite;
use chrono::Utc;

// Import feedback storage - binaries can access parent crate modules
// We'll need to make sure the module is accessible
// For now, we'll duplicate the essential parts or use a shared approach
// Since this is a binary, we'll access the storage directly
use std::sync::Arc;
use std::sync::Mutex;
use rusqlite::Connection;
use rusqlite::Result as SqliteResult;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-implement FeedbackStorage for the binary (or we could make it a library)
// For simplicity, we'll access the database directly
struct FeedbackStorage {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FeedbackEntry {
    id: i64,
    query: String,
    document_id: String,
    is_relevant: bool,
    timestamp: DateTime<Utc>,
    session_id: Option<String>,
}

impl FeedbackStorage {
    fn new(db_path: Option<PathBuf>) -> SqliteResult<Self> {
        use dirs;
        let db_path = db_path.unwrap_or_else(|| {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("pagi-digital-twin");
            std::fs::create_dir_all(&data_dir).ok();
            data_dir.join("search_feedback.db")
        });

        let conn = Connection::open(&db_path)?;
        
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

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn get_feedback_count(&self) -> SqliteResult<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM search_feedback",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn get_feedback_grouped_by_query(&self) -> SqliteResult<HashMap<String, Vec<FeedbackEntry>>> {
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

        let mut grouped: HashMap<String, Vec<FeedbackEntry>> = HashMap::new();
        for entry in entries {
            grouped.entry(entry.query.clone()).or_insert_with(Vec::new).push(entry);
        }

        Ok(grouped)
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    /// JSONL format for HuggingFace SentenceTransformers
    Jsonl,
    /// TSV format (query, document, label)
    Tsv,
    /// CSV format
    Csv,
}

/// Training data entry for Cross-Encoder fine-tuning
#[derive(Debug, Serialize, Deserialize)]
struct TrainingExample {
    /// Query text
    query: String,
    /// Document text (snippet or full content)
    document: String,
    /// Relevance label (1 for relevant, 0 for irrelevant)
    label: i32,
    /// Document ID for reference
    document_id: String,
}

/// Pair of examples for contrastive learning
#[derive(Debug)]
struct TrainingPair {
    positive: TrainingExample,
    negative: TrainingExample,
    query: String,
}

#[derive(Parser)]
#[command(name = "train_reranker")]
#[command(about = "Process search feedback and prepare training data for Cross-Encoder fine-tuning")]
struct Args {
    /// Path to feedback database (default: data directory)
    #[arg(short, long)]
    db_path: Option<PathBuf>,

    /// Output file path
    #[arg(short, long, default_value = "training_data.jsonl")]
    output: PathBuf,

    /// Output format
    #[arg(short, long, value_enum, default_value = "jsonl")]
    format: OutputFormat,

    /// Minimum feedback entries per query to include
    #[arg(short, long, default_value = "2")]
    min_entries: usize,

    /// Include document content (requires Qdrant access)
    #[arg(long)]
    include_content: bool,

    /// Trigger training after export (requires Python/HuggingFace)
    #[arg(long)]
    train: bool,

    /// Base model for fine-tuning
    #[arg(long, default_value = "cross-encoder/ms-marco-MiniLM-L-6-v2")]
    base_model: String,
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    info!("Starting Cross-Encoder training data preparation");

    // Initialize feedback storage
    let storage = FeedbackStorage::new(args.db_path.clone())
        .context("Failed to initialize feedback storage")?;

    let feedback_count = storage.get_feedback_count()?;
    info!(count = feedback_count, "Total feedback entries in database");

    if feedback_count == 0 {
        warn!("No feedback entries found. Please collect feedback first.");
        return Ok(());
    }

    // Load all feedback grouped by query
    let grouped_feedback = storage.get_feedback_grouped_by_query()?;
    info!(queries = grouped_feedback.len(), "Feedback grouped by query");

    // Process feedback into training examples
    let mut training_examples = Vec::new();
    let mut training_pairs = Vec::new();

    for (query, entries) in grouped_feedback {
        if entries.len() < args.min_entries {
            continue;
        }

        // Separate positive and negative examples
        let positive: Vec<_> = entries.iter()
            .filter(|e| e.is_relevant)
            .collect();
        let negative: Vec<_> = entries.iter()
            .filter(|e| !e.is_relevant)
            .collect();

        // Create individual training examples
        for entry in &entries {
            // For now, we'll use document_id as document text
            // In production, you'd fetch actual content from Qdrant
            let document_text = if args.include_content {
                // TODO: Fetch actual content from Qdrant using document_id
                format!("Document: {}", entry.document_id)
            } else {
                entry.document_id.clone()
            };

            training_examples.push(TrainingExample {
                query: entry.query.clone(),
                document: document_text,
                label: if entry.is_relevant { 1 } else { 0 },
                document_id: entry.document_id.clone(),
            });
        }

        // Create contrastive pairs (positive + negative for same query)
        for pos_entry in &positive {
            for neg_entry in &negative {
                let pos_doc = if args.include_content {
                    format!("Document: {}", pos_entry.document_id)
                } else {
                    pos_entry.document_id.clone()
                };

                let neg_doc = if args.include_content {
                    format!("Document: {}", neg_entry.document_id)
                } else {
                    neg_entry.document_id.clone()
                };

                training_pairs.push(TrainingPair {
                    positive: TrainingExample {
                        query: query.clone(),
                        document: pos_doc,
                        label: 1,
                        document_id: pos_entry.document_id.clone(),
                    },
                    negative: TrainingExample {
                        query: query.clone(),
                        document: neg_doc,
                        label: 0,
                        document_id: neg_entry.document_id.clone(),
                    },
                    query: query.clone(),
                });
            }
        }
    }

    info!(
        examples = training_examples.len(),
        pairs = training_pairs.len(),
        "Generated training data"
    );

    // Export to requested format
    match args.format {
        OutputFormat::Jsonl => export_jsonl(&args.output, &training_examples, &training_pairs)?,
        OutputFormat::Tsv => export_tsv(&args.output, &training_examples)?,
        OutputFormat::Csv => export_csv(&args.output, &training_examples)?,
    }

    info!(path = %args.output.display(), "Training data exported");

    // Optionally trigger training
    if args.train {
        info!("Training mode requested - this requires Python/HuggingFace setup");
        warn!("Automatic training not yet implemented. Please use the exported data with HuggingFace SentenceTransformers.");
        warn!("Example command:");
        warn!("  python -m sentence_transformers.cross_encoder.train --base_model {} --train_data {}", 
              args.base_model, args.output.display());
    }

    Ok(())
}

/// Export training data to JSONL format
fn export_jsonl(
    output_path: &PathBuf,
    examples: &[TrainingExample],
    pairs: &[TrainingPair],
) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
    let mut writer = std::io::BufWriter::new(file);

    // Write individual examples
    for example in examples {
        let json = serde_json::to_string(example)?;
        writeln!(writer, "{}", json)?;
    }

    // Write pairs for contrastive learning
    for pair in pairs {
        // Write positive example
        let pos_json = serde_json::to_string(&pair.positive)?;
        writeln!(writer, "{}", pos_json)?;

        // Write negative example
        let neg_json = serde_json::to_string(&pair.negative)?;
        writeln!(writer, "{}", neg_json)?;
    }

    writer.flush()?;
    Ok(())
}

/// Export training data to TSV format
fn export_tsv(output_path: &PathBuf, examples: &[TrainingExample]) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
    let mut writer = std::io::BufWriter::new(file);

    // Write header
    writeln!(writer, "query\tdocument\tlabel\tdocument_id")?;

    // Write examples
    for example in examples {
        // Escape tabs in text
        let query = example.query.replace('\t', " ");
        let document = example.document.replace('\t', " ");
        
        writeln!(
            writer,
            "{}\t{}\t{}\t{}",
            query, document, example.label, example.document_id
        )?;
    }

    writer.flush()?;
    Ok(())
}

/// Export training data to CSV format
fn export_csv(output_path: &PathBuf, examples: &[TrainingExample]) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
    let mut writer = std::io::BufWriter::new(file);

    // Write header
    writeln!(writer, "query,document,label,document_id")?;

    // Write examples
    for example in examples {
        // Escape commas and quotes in CSV
        let query = escape_csv(&example.query);
        let document = escape_csv(&example.document);
        let doc_id = escape_csv(&example.document_id);

        writeln!(
            writer,
            "{},{},{},{}",
            query, document, example.label, doc_id
        )?;
    }

    writer.flush()?;
    Ok(())
}

/// Escape CSV special characters
fn escape_csv(text: &str) -> String {
    if text.contains(',') || text.contains('"') || text.contains('\n') {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}
