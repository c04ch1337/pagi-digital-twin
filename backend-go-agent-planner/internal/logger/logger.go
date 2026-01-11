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

// Fatalf logs an error message and exits the program with status code 1.
// This provides Fatalf-like functionality for slog.Logger.
func Fatalf(logger *slog.Logger, msg string, args ...any) {
	logger.Error(msg, args...)
	os.Exit(1)
}

// LogCircuitBreakerStateChange logs a structured event whenever a circuit breaker
// transitions between states.
//
// Typical transitions: closed -> open, open -> half-open, half-open -> closed.
func LogCircuitBreakerStateChange(logger *slog.Logger, breakerName string, fromState string, toState string) {
	if logger == nil {
		logger = defaultLogger
	}
	logger.Warn(
		"circuit_breaker_state_change",
		"breaker", breakerName,
		"from", fromState,
		"to", toState,
	)
}
