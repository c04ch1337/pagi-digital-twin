package audit

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	_ "github.com/mattn/go-sqlite3"
)

// AuditDB is a lightweight, embedded audit log store for the Agent Planner.
//
// It writes an append-only chronological record of key AgentLoop events to SQLite.
type AuditDB struct {
	db *sql.DB
}

const createTableSQL = `
CREATE TABLE IF NOT EXISTS audit_log (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	trace_id TEXT,
	session_id TEXT,
	timestamp DATETIME NOT NULL,
	event_type TEXT NOT NULL,
	data TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_log_trace_id ON audit_log(trace_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_session_id ON audit_log(session_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
`

// NewAuditDB opens/creates the SQLite database at dbPath and ensures the schema exists.
func NewAuditDB(dbPath string) (*AuditDB, error) {
	if dbPath == "" {
		dbPath = "./pagi_audit.db"
	}

	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		return nil, fmt.Errorf("open sqlite: %w", err)
	}

	// SQLite works best with a single writer connection.
	db.SetMaxOpenConns(1)
	db.SetMaxIdleConns(1)

	if err := db.Ping(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("ping sqlite: %w", err)
	}

	if _, err := db.Exec(createTableSQL); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("create schema: %w", err)
	}

	return &AuditDB{db: db}, nil
}

func (a *AuditDB) Close() error {
	if a == nil || a.db == nil {
		return nil
	}
	return a.db.Close()
}

// RecordStep inserts a single audit log row.
//
// - traceID: the request correlation ID (X-Trace-ID)
// - sessionID: agent session identifier
// - eventType: e.g. PLAN_START, TOOL_CALL, PLAN_END
// - data: JSON-encoded payload (best-effort)
func (a *AuditDB) RecordStep(ctx context.Context, traceID, sessionID, eventType string, data any) error {
	if a == nil || a.db == nil {
		return nil
	}

	var payload string
	if data != nil {
		b, err := json.Marshal(data)
		if err != nil {
			payload = fmt.Sprintf(`{"marshal_error":%q}`, err.Error())
		} else {
			payload = string(b)
		}
	}

	_, err := a.db.ExecContext(
		ctx,
		`INSERT INTO audit_log (trace_id, session_id, timestamp, event_type, data)
		 VALUES (?, ?, ?, ?, ?)`,
		traceID,
		sessionID,
		time.Now().UTC(),
		eventType,
		payload,
	)
	if err != nil {
		return fmt.Errorf("insert audit_log: %w", err)
	}

	return nil
}
