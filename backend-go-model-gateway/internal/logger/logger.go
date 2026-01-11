package logger

import (
	"context"
	"log/slog"
	"os"
)

// contextKey is an unexported type for context keys.
type contextKey string

// TraceIDKey is the context key (and canonical header name) for the Trace ID.
const TraceIDKey contextKey = "X-Trace-ID"

var defaultLogger = slog.New(slog.NewTextHandler(os.Stdout, nil))

// NewContextLogger creates a logger that always includes the trace_id from the context, if present.
func NewContextLogger(ctx context.Context) *slog.Logger {
	traceID, ok := ctx.Value(TraceIDKey).(string)
	if !ok || traceID == "" {
		return defaultLogger
	}
	return defaultLogger.With("trace_id", traceID)
}
