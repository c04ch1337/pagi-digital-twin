package service

import (
	"context"
	"strings"

	"backend-go-model-gateway/internal/logger"

	"google.golang.org/grpc/metadata"
)

// ContextWithTraceIDFromIncomingGRPC extracts X-Trace-ID from incoming gRPC metadata
// and injects it into the returned context for downstream logging.
func ContextWithTraceIDFromIncomingGRPC(ctx context.Context) context.Context {
	traceID := ""
	if md, ok := metadata.FromIncomingContext(ctx); ok {
		key := strings.ToLower(string(logger.TraceIDKey))
		if ids := md.Get(key); len(ids) > 0 {
			traceID = ids[0]
		}
	}
	if strings.TrimSpace(traceID) == "" {
		return ctx
	}
	return context.WithValue(ctx, logger.TraceIDKey, traceID)
}
