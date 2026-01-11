package main

import (
	"context"
	"log"
	"math"
	"time"

	pb "backend-go-model-gateway/proto/proto"

	"go.opentelemetry.io/contrib/instrumentation/google.golang.org/grpc/otelgrpc"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// VectorQueryRequest defines the input for a vector search.
//
// This is intentionally small for the initial integration phase.
// The actual Vector DB implementation (Pinecone/Weaviate/etc.) will likely
// expand this with filters, namespaces, and/or an embedding vector.
type VectorQueryRequest struct {
	QueryText string `json:"query_text"`
	TopK      int    `json:"top_k"`
	// KnowledgeBases are conceptual KB identifiers requested by the caller.
	//
	// NOTE: This is a temporary stand-in for a future protobuf field, allowing the
	// gateway to simulate multi-KB retrieval while the external request schema is
	// still fixed.
	KnowledgeBases []string `json:"knowledge_bases,omitempty"`
	// Placeholder for embedding vector if needed later.
	// Embedding []float32 `json:"embedding,omitempty"`
}

// VectorQueryMatch defines a single search result.
type VectorQueryMatch struct {
	ID            string  `json:"id"`
	Score         float64 `json:"score"`
	Text          string  `json:"text"`
	Source        string  `json:"source"`
	KnowledgeBase string  `json:"knowledge_base"`
}

// RAGContextClient provides multi-KB RAG context for the gateway.
type RAGContextClient interface {
	GetContext(ctx context.Context, req VectorQueryRequest) ([]VectorQueryMatch, error)
}

// RAGGRPCClient implements RAG retrieval by calling the Python Memory Service over gRPC.
type RAGGRPCClient struct {
	conn   *grpc.ClientConn
	client pb.ModelGatewayClient
}

func NewRAGGRPCClient(ctx context.Context) (*RAGGRPCClient, error) {
	addr := getEnv("RAG_GRPC_ADDR", "localhost:50052")

	conn, err := grpc.DialContext(
		ctx,
		addr,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithStatsHandler(otelgrpc.NewClientHandler()),
	)
	if err != nil {
		return nil, err
	}

	return &RAGGRPCClient{conn: conn, client: pb.NewModelGatewayClient(conn)}, nil
}

func (c *RAGGRPCClient) Close() error {
	if c == nil || c.conn == nil {
		return nil
	}
	return c.conn.Close()
}

func (c *RAGGRPCClient) GetContext(ctx context.Context, req VectorQueryRequest) ([]VectorQueryMatch, error) {
	if req.TopK <= 0 {
		req.TopK = 2
	}

	resp, err := c.client.GetRAGContext(ctx, &pb.RAGContextRequest{
		Query:          req.QueryText,
		TopK:           int32(req.TopK),
		KnowledgeBases: req.KnowledgeBases,
	})
	if err != nil {
		return nil, err
	}

	matches := make([]VectorQueryMatch, 0, len(resp.GetMatches()))
	for _, m := range resp.GetMatches() {
		d := m.GetDistance()
		score := 0.0
		if d >= 0 {
			score = 1.0 / (1.0 + math.Abs(d))
		}
		matches = append(matches, VectorQueryMatch{
			ID:            m.GetId(),
			Score:         score,
			Text:          m.GetText(),
			Source:        m.GetSource(),
			KnowledgeBase: m.GetKnowledgeBase(),
		})
	}

	log.Printf(
		`{"timestamp":"%s","level":"info","service":"%s","component":"RAGGRPCClient","method":"GetContext","rag_addr":%q,"query_text":%q,"top_k":%d,"match_count":%d}`,
		time.Now().Format(time.RFC3339Nano), SERVICE_NAME, getEnv("RAG_GRPC_ADDR", "localhost:50052"), req.QueryText, req.TopK, len(matches),
	)

	return matches, nil
}
