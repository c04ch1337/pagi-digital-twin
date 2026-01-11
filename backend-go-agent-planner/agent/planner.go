package agent

import (
	"bytes"
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"backend-go-agent-planner/audit"
	"backend-go-agent-planner/internal/logger"
	pb "backend-go-model-gateway/proto/proto"

	"github.com/go-redis/redis/v8"
	"github.com/sony/gobreaker"

	"go.opentelemetry.io/contrib/instrumentation/google.golang.org/grpc/otelgrpc"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/codes"
	"go.opentelemetry.io/otel/metric"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

func loadMTLSClientCredsForAddr(addr string) (credentials.TransportCredentials, bool, error) {
	clientCertPath := os.Getenv("TLS_CLIENT_CERT_PATH")
	clientKeyPath := os.Getenv("TLS_CLIENT_KEY_PATH")
	caCertPath := os.Getenv("TLS_CA_CERT_PATH")

	// Allow non-TLS local dev unless explicitly configured.
	if clientCertPath == "" && clientKeyPath == "" && caCertPath == "" {
		return nil, false, nil
	}
	if clientCertPath == "" || clientKeyPath == "" || caCertPath == "" {
		return nil, false, fmt.Errorf("mTLS misconfigured: TLS_CLIENT_CERT_PATH, TLS_CLIENT_KEY_PATH, TLS_CA_CERT_PATH must all be set")
	}

	clientCert, err := tls.LoadX509KeyPair(clientCertPath, clientKeyPath)
	if err != nil {
		return nil, false, fmt.Errorf("load client keypair (%s, %s): %w", filepath.Clean(clientCertPath), filepath.Clean(clientKeyPath), err)
	}

	caPEM, err := os.ReadFile(caCertPath)
	if err != nil {
		return nil, false, fmt.Errorf("read CA cert (%s): %w", filepath.Clean(caCertPath), err)
	}
	caPool := x509.NewCertPool()
	if ok := caPool.AppendCertsFromPEM(caPEM); !ok {
		return nil, false, fmt.Errorf("append CA certs from PEM (%s): no certs parsed", filepath.Clean(caCertPath))
	}

	host := addr
	if i := strings.LastIndex(addr, ":"); i > 0 {
		host = addr[:i]
	}
	// Hostname verification must match the server certificate's SAN/CN.
	serverName := os.Getenv("TLS_SERVER_NAME")
	if strings.TrimSpace(serverName) == "" {
		serverName = host
	}

	conf := &tls.Config{
		MinVersion:   tls.VersionTLS12,
		Certificates: []tls.Certificate{clientCert},
		RootCAs:      caPool,
		ServerName:   serverName,
		NextProtos:   []string{"h2"},
	}

	return credentials.NewTLS(conf), true, nil
}

type Config struct {
	ModelGatewayAddr    string
	MemoryServiceAddr   string
	MemoryServiceHTTP   string
	RustSandboxGRPCAddr string
	RustSandboxHTTPURL  string
	AuditDBPath         string
	RedisAddr           string

	MaxTurns int
	TopK     int
	KBs      []string
}

// Resource represents a structured, optional multi-modal input reference.
//
// This is intentionally "agnostic" and currently passed through to the Model
// Gateway without affecting planning logic.
type Resource struct {
	Type string `json:"type"`
	URI  string `json:"uri"`
}

func ConfigFromEnv() Config {
	maxTurns := 3
	if v := os.Getenv("AGENT_MAX_TURNS"); v != "" {
		fmt.Sscanf(v, "%d", &maxTurns)
	}

	topK := 3
	if v := os.Getenv("AGENT_RAG_TOP_K"); v != "" {
		fmt.Sscanf(v, "%d", &topK)
	}

	return Config{
		ModelGatewayAddr:    getenv("MODEL_GATEWAY_ADDR", "localhost:50051"),
		MemoryServiceAddr:   getenv("MEMORY_GRPC_ADDR", "localhost:50052"),
		MemoryServiceHTTP:   getenv("MEMORY_URL", "http://localhost:8003"),
		RustSandboxGRPCAddr: getenv("RUST_SANDBOX_GRPC_ADDR", "localhost:50053"),
		RustSandboxHTTPURL:  getenv("RUST_SANDBOX_URL", "http://localhost:8001"),
		AuditDBPath:         getenv("PAGI_AUDIT_DB_PATH", "./pagi_audit.db"),
		RedisAddr:           getenv("REDIS_ADDR", "localhost:6379"),
		MaxTurns:            maxTurns,
		TopK:                topK,
		// Include Mind-KB so the planner can retrieve evolving playbooks via the existing RAG call.
		KBs: []string{"Mind-KB", "Domain-KB", "Body-KB", "Soul-KB"},
	}
}

func getenv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

type Planner struct {
	cfg Config

	modelConn  *grpc.ClientConn
	memoryConn *grpc.ClientConn
	rustConn   *grpc.ClientConn

	modelClient  pb.ModelGatewayClient
	memoryClient pb.ModelGatewayClient
	toolClient   pb.ToolServiceClient

	// Circuit breakers to prevent cascading failures when downstream dependencies
	// are unhealthy or slow.
	modelBreaker  *gobreaker.CircuitBreaker
	memoryBreaker *gobreaker.CircuitBreaker

	httpClient *http.Client
	auditDB    *audit.AuditDB
	redis      *redis.Client
}

const notificationsChannel = "pagi_notifications"

var (
	metricsOnce   sync.Once
	planCounter   metric.Int64Counter
	loopDurationS metric.Float64Histogram
)

func initMetrics() {
	metricsOnce.Do(func() {
		m := otel.Meter("backend-go-agent-planner")
		var err error
		planCounter, err = m.Int64Counter(
			"agent_plan_total",
			metric.WithDescription("Count of agent planner executions (success/failure)."),
			metric.WithUnit("1"),
		)
		if err != nil {
			planCounter = nil
		}
		loopDurationS, err = m.Float64Histogram(
			"agent_loop_duration_seconds",
			metric.WithDescription("End-to-end AgentLoop duration in seconds."),
			metric.WithUnit("s"),
		)
		if err != nil {
			loopDurationS = nil
		}
	})
}

func NewPlanner(ctx context.Context, cfg Config) (*Planner, error) {
	lg := logger.NewContextLogger(ctx)

	dialInsecure := func(ctx context.Context, addr string) (*grpc.ClientConn, error) {
		return grpc.DialContext(
			ctx,
			addr,
			grpc.WithTransportCredentials(insecure.NewCredentials()),
			grpc.WithStatsHandler(otelgrpc.NewClientHandler()),
		)
	}

	dialModelGateway := func(ctx context.Context, addr string) (*grpc.ClientConn, error) {
		if creds, enabled, err := loadMTLSClientCredsForAddr(addr); err != nil {
			return nil, err
		} else if enabled {
			lg.Info("mtls_enabled_for_model_gateway", "addr", addr)
			return grpc.DialContext(
				ctx,
				addr,
				grpc.WithTransportCredentials(creds),
				grpc.WithStatsHandler(otelgrpc.NewClientHandler()),
			)
		}
		lg.Warn("mtls_not_enabled_for_model_gateway", "addr", addr)
		return dialInsecure(ctx, addr)
	}

	modelConn, err := dialModelGateway(ctx, cfg.ModelGatewayAddr)
	if err != nil {
		return nil, fmt.Errorf("dial model gateway: %w", err)
	}

	memoryConn, err := dialInsecure(ctx, cfg.MemoryServiceAddr)
	if err != nil {
		_ = modelConn.Close()
		return nil, fmt.Errorf("dial memory service: %w", err)
	}

	rustConn, err := dialInsecure(ctx, cfg.RustSandboxGRPCAddr)
	if err != nil {
		_ = memoryConn.Close()
		_ = modelConn.Close()
		return nil, fmt.Errorf("dial rust sandbox: %w", err)
	}

	auditDB, err := audit.NewAuditDB(cfg.AuditDBPath)
	if err != nil {
		_ = rustConn.Close()
		_ = memoryConn.Close()
		_ = modelConn.Close()
		return nil, fmt.Errorf("init audit db: %w", err)
	}

	redisClient := redis.NewClient(&redis.Options{Addr: cfg.RedisAddr})
	if err := redisClient.Ping(ctx).Err(); err != nil {
		lg.Warn("redis_unavailable", "addr", cfg.RedisAddr, "error", err)
		_ = redisClient.Close()
		redisClient = nil
	}

	// Circuit breaker defaults (production-like):
	// - Open after 5 consecutive failures.
	// - Stay open for 30s, then allow 1 request (half-open) to probe recovery.
	newBreaker := func(name string) *gobreaker.CircuitBreaker {
		return gobreaker.NewCircuitBreaker(gobreaker.Settings{
			Name:        name,
			MaxRequests: 1,
			Timeout:     30 * time.Second,
			ReadyToTrip: func(counts gobreaker.Counts) bool {
				return counts.ConsecutiveFailures >= 5
			},
			OnStateChange: func(name string, from gobreaker.State, to gobreaker.State) {
				logger.LogCircuitBreakerStateChange(lg, name, from.String(), to.String())
			},
		})
	}

	return &Planner{
		cfg:           cfg,
		modelConn:     modelConn,
		memoryConn:    memoryConn,
		rustConn:      rustConn,
		modelClient:   pb.NewModelGatewayClient(modelConn),
		memoryClient:  pb.NewModelGatewayClient(memoryConn),
		toolClient:    pb.NewToolServiceClient(rustConn),
		modelBreaker:  newBreaker("model_gateway"),
		memoryBreaker: newBreaker("memory_service"),
		httpClient:    &http.Client{Timeout: 10 * time.Second},
		auditDB:       auditDB,
		redis:         redisClient,
	}, nil
}

func (p *Planner) callModelGatewayGetPlan(ctx context.Context, prompt string, resources []Resource) (*pb.PlanResponse, error) {
	if p == nil || p.modelClient == nil {
		return nil, fmt.Errorf("model client is nil")
	}

	call := func() (*pb.PlanResponse, error) {
		pbResources := make([]*pb.Resource, 0, len(resources))
		for _, r := range resources {
			// Validation is done at the HTTP boundary; treat empty fields as best-effort.
			if strings.TrimSpace(r.Type) == "" || strings.TrimSpace(r.URI) == "" {
				continue
			}
			pbResources = append(pbResources, &pb.Resource{Type: r.Type, Uri: r.URI})
		}

		// Per-request timeout (separate from breaker open timeout).
		// LLM generation can be slow; avoid premature timeouts that would cause false
		// positives for the circuit breaker.
		timeout := 60 * time.Second
		logger.NewContextLogger(ctx).Info("grpc_timeout_applied", "dependency", "model_gateway", "timeout_seconds", int(timeout.Seconds()))
		ctx2, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()
		return p.modelClient.GetPlan(ctx2, &pb.PlanRequest{Prompt: prompt, Resources: pbResources})
	}

	if p.modelBreaker == nil {
		return call()
	}

	respAny, err := p.modelBreaker.Execute(func() (any, error) {
		return call()
	})
	if err != nil {
		if errors.Is(err, gobreaker.ErrOpenState) || errors.Is(err, gobreaker.ErrTooManyRequests) {
			return nil, fmt.Errorf("model gateway circuit open: %w", err)
		}
		return nil, err
	}
	resp, _ := respAny.(*pb.PlanResponse)
	if resp == nil {
		return nil, fmt.Errorf("unexpected response type from model gateway")
	}
	return resp, nil
}

func (p *Planner) callMemoryGetRAGContext(ctx context.Context, query string) (*pb.RAGContextResponse, error) {
	if p == nil || p.memoryClient == nil {
		return nil, fmt.Errorf("memory client is nil")
	}

	call := func() (*pb.RAGContextResponse, error) {
		// Per-request timeout (separate from breaker open timeout).
		// RAG calls can be moderately slow; use a larger timeout to avoid tripping
		// the circuit breaker on transient slowness.
		timeout := 30 * time.Second
		logger.NewContextLogger(ctx).Info("grpc_timeout_applied", "dependency", "memory_service", "timeout_seconds", int(timeout.Seconds()))
		ctx2, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()
		return p.memoryClient.GetRAGContext(ctx2, &pb.RAGContextRequest{
			Query:          query,
			TopK:           int32(p.cfg.TopK),
			KnowledgeBases: p.cfg.KBs,
		})
	}

	if p.memoryBreaker == nil {
		return call()
	}

	respAny, err := p.memoryBreaker.Execute(func() (any, error) {
		return call()
	})
	if err != nil {
		if errors.Is(err, gobreaker.ErrOpenState) || errors.Is(err, gobreaker.ErrTooManyRequests) {
			return nil, fmt.Errorf("memory service circuit open: %w", err)
		}
		return nil, err
	}
	resp, _ := respAny.(*pb.RAGContextResponse)
	if resp == nil {
		return nil, fmt.Errorf("unexpected response type from memory service")
	}
	return resp, nil
}

func (p *Planner) Close() {
	if p == nil {
		return
	}
	if p.modelConn != nil {
		_ = p.modelConn.Close()
	}
	if p.memoryConn != nil {
		_ = p.memoryConn.Close()
	}
	if p.rustConn != nil {
		_ = p.rustConn.Close()
	}
	if p.auditDB != nil {
		_ = p.auditDB.Close()
	}
	if p.redis != nil {
		_ = p.redis.Close()
	}
}

type ToolCall struct {
	Name string         `json:"name"`
	Args map[string]any `json:"args"`
	Raw  map[string]any `json:"-"`
}

func injectTraceIDToOutgoingGRPC(ctx context.Context) context.Context {
	traceID, _ := ctx.Value(logger.TraceIDKey).(string)
	if strings.TrimSpace(traceID) == "" {
		return ctx
	}
	// gRPC metadata keys must be lowercase.
	key := strings.ToLower(string(logger.TraceIDKey))
	return metadata.AppendToOutgoingContext(ctx, key, traceID)
}

func (p *Planner) RecordStep(ctx context.Context, sessionID, eventType string, data any) error {
	if p == nil || p.auditDB == nil {
		return nil
	}
	traceID, _ := ctx.Value(logger.TraceIDKey).(string)
	return p.auditDB.RecordStep(ctx, traceID, sessionID, eventType, data)
}

func (p *Planner) PublishStatus(ctx context.Context, sessionID string, status string) error {
	if p == nil || p.redis == nil {
		return nil
	}
	traceID, _ := ctx.Value(logger.TraceIDKey).(string)
	payload := map[string]any{
		"trace_id":   traceID,
		"session_id": sessionID,
		"status":     status,
		"timestamp":  time.Now().UTC().Format(time.RFC3339Nano),
	}
	b, _ := json.Marshal(payload)
	return p.redis.Publish(ctx, notificationsChannel, string(b)).Err()
}

func (p *Planner) PublishNotification(ctx context.Context, sessionID string, result string) error {
	if p == nil || p.redis == nil {
		return nil
	}
	traceID, _ := ctx.Value(logger.TraceIDKey).(string)
	payload := map[string]any{
		"trace_id":   traceID,
		"session_id": sessionID,
		"result":     result,
		"timestamp":  time.Now().UTC().Format(time.RFC3339Nano),
	}
	b, _ := json.Marshal(payload)
	return p.redis.Publish(ctx, notificationsChannel, string(b)).Err()
}

// AgentLoop orchestrates Memory -> Plan -> (Tool?) -> Persist, repeating up to MaxTurns.

func (p *Planner) AgentLoop(ctx context.Context, prompt string, sessionID string, resources []Resource) (result string, err error) {
	initMetrics()

	tracer := otel.Tracer("backend-go-agent-planner")
	ctx, span := tracer.Start(ctx, "AgentLoopExecution")
	span.SetAttributes(
		attribute.String("session_id", sessionID),
		attribute.Int("resource_count", len(resources)),
	)
	start := time.Now()
	defer func() {
		if loopDurationS != nil {
			loopDurationS.Record(ctx, time.Since(start).Seconds())
		}
		if planCounter != nil {
			outcome := "success"
			if err != nil {
				outcome = "error"
			}
			planCounter.Add(ctx, 1, metric.WithAttributes(attribute.String("outcome", outcome)))
		}

		if err != nil {
			span.RecordError(err)
			span.SetStatus(codes.Error, err.Error())
		} else {
			span.SetStatus(codes.Ok, "")
		}
		span.End()
	}()

	ctx = injectTraceIDToOutgoingGRPC(ctx)
	lg := logger.NewContextLogger(ctx)

	basePrompt := prompt
	_ = p.RecordStep(ctx, sessionID, "PLAN_START", map[string]any{"prompt": basePrompt, "resources": resources, "max_turns": p.cfg.MaxTurns, "top_k": p.cfg.TopK, "kbs": p.cfg.KBs})
	_ = p.PublishStatus(ctx, sessionID, "STARTED")
	// Collect a per-run playbook sequence (user prompt + tool-plan/tool-result pairs + final answer).
	// This is persisted to Mind-KB only on successful completion.
	playbookSeq := []map[string]string{{"role": "user", "content": basePrompt}}
	hadToolStep := false

	maxTurns := p.cfg.MaxTurns
	if maxTurns <= 0 {
		maxTurns = 3
	}

	for turn := 1; turn <= maxTurns; turn++ {
		span.SetAttributes(attribute.Int("turn", turn))

		// 1) Session history (Episodic/Heart) via Memory HTTP API.
		var history []map[string]any
		{
			ctxStep, stepSpan := tracer.Start(ctx, "MemoryAccess.SessionHistory")
			history, _ = p.fetchSessionHistory(ctxStep, sessionID)
			stepSpan.End()
		}

		// 2) RAG context (Domain/Body/Soul) via Memory gRPC.
		var rag *pb.RAGContextResponse
		{
			ctxStep, stepSpan := tracer.Start(ctx, "MemoryAccess.RAGContext")
			rag, err = p.callMemoryGetRAGContext(ctxStep, prompt)
			if err != nil {
				stepSpan.RecordError(err)
			}
			stepSpan.End()
		}
		if err != nil {
			lg.Warn("rag_context_unavailable", "error", err)
			rag = nil
		}

		plannerInput := buildPlannerPrompt(prompt, history, rag)

		// 3) Planning via Model Gateway.
		var planResp *pb.PlanResponse
		{
			ctxStep, stepSpan := tracer.Start(ctx, "PlanGeneration")
			planResp, err = p.callModelGatewayGetPlan(ctxStep, plannerInput, resources)
			if err != nil {
				stepSpan.RecordError(err)
			}
			stepSpan.End()
		}
		if err != nil {
			_ = p.RecordStep(ctx, sessionID, "PLAN_ERROR", map[string]any{"error": err.Error()})
			return "", fmt.Errorf("GetPlan: %w", err)
		}
		_ = p.RecordStep(ctx, sessionID, "PLAN_MODEL_RESPONSE", map[string]any{"plan": planResp.GetPlan()})

		toolCall := tryParseToolCall(planResp.GetPlan())
		if toolCall == nil {
			// Successful completion path (non-tool-call final answer).
			playbookSeq = append(playbookSeq, map[string]string{"role": "assistant", "content": planResp.GetPlan()})
			_ = p.RecordStep(ctx, sessionID, "PLAN_END", map[string]any{"result": planResp.GetPlan()})
			if hadToolStep {
				_ = p.storePlaybook(ctx, sessionID, basePrompt, playbookSeq)
			}
			_ = p.storeSessionDelta(ctx, sessionID, prompt, planResp.GetPlan())
			_ = p.PublishNotification(ctx, sessionID, planResp.GetPlan())
			_ = p.PublishStatus(ctx, sessionID, "COMPLETED")
			return planResp.GetPlan(), nil
		}

		_ = p.RecordStep(ctx, sessionID, "TOOL_CALL", map[string]any{"tool": toolCall.Name, "args": toolCall.Args})

		// 4) Tool execution via Rust sandbox ToolService over gRPC.
		var toolOut string
		{
			ctxStep, stepSpan := tracer.Start(ctx, "ToolCallExecution")
			stepSpan.SetAttributes(attribute.String("tool.name", toolCall.Name))
			toolOut, err = p.executeTool(ctxStep, toolCall.Name, toolCall.Args)
			if err != nil {
				stepSpan.RecordError(err)
			}
			stepSpan.End()
		}
		if err != nil {
			_ = p.RecordStep(ctx, sessionID, "TOOL_ERROR", map[string]any{"tool": toolCall.Name, "error": err.Error()})
			// Feed tool error back into the loop.
			prompt = prompt + "\n\nTool error: " + err.Error()
			continue
		}
		_ = p.RecordStep(ctx, sessionID, "TOOL_RESULT", map[string]any{"tool": toolCall.Name, "output": toolOut})

		hadToolStep = true
		playbookSeq = append(playbookSeq, map[string]string{"role": "assistant", "content": planResp.GetPlan()})
		playbookSeq = append(playbookSeq, map[string]string{"role": "tool_result", "content": toolOut})

		// 5) Loop/feedback.
		prompt = buildFollowupPrompt(prompt, planResp.GetPlan(), toolOut)
		_ = p.storeSessionDelta(ctx, sessionID, "[tool-plan]", planResp.GetPlan())
		_ = p.storeSessionDelta(ctx, sessionID, "[tool-output]", toolOut)
	}

	return "Max turns reached; unable to complete request.", nil
}

func buildPlannerPrompt(userPrompt string, history []map[string]any, rag *pb.RAGContextResponse) string {
	var b strings.Builder
	b.WriteString("<session_history>\n")
	for _, m := range history {
		role, _ := m["role"].(string)
		content, _ := m["content"].(string)
		if role != "" || content != "" {
			b.WriteString(role + ": " + content + "\n")
		}
	}
	b.WriteString("</session_history>\n\n")

	b.WriteString("<rag_context>\n")
	if rag != nil {
		for _, m := range rag.GetMatches() {
			b.WriteString("**" + m.GetKnowledgeBase() + "**\n")
			b.WriteString("ID: " + m.GetId() + "\n")
			b.WriteString("Text: " + m.GetText() + "\n---\n")
		}
	}
	b.WriteString("</rag_context>\n\n")

	b.WriteString("<user_prompt>\n")
	b.WriteString(userPrompt)
	b.WriteString("\n</user_prompt>\n")
	return b.String()
}

func buildFollowupPrompt(originalPrompt, plan, toolResult string) string {
	return originalPrompt + "\n\n<plan>\n" + plan + "\n</plan>\n\n<tool_result>\n" + toolResult + "\n</tool_result>\n"
}

func tryParseToolCall(planJSON string) *ToolCall {
	// Minimal parsing strategy:
	// - if JSON contains {"tool": {"name": ..., "args": {...}}} treat it as tool call.
	var raw map[string]any
	if err := json.Unmarshal([]byte(planJSON), &raw); err != nil {
		return nil
	}
	toolObj, ok := raw["tool"].(map[string]any)
	if !ok {
		return nil
	}
	name, _ := toolObj["name"].(string)
	args, _ := toolObj["args"].(map[string]any)
	if strings.TrimSpace(name) == "" {
		return nil
	}
	return &ToolCall{Name: name, Args: args, Raw: raw}
}

func (p *Planner) fetchSessionHistory(ctx context.Context, sessionID string) ([]map[string]any, error) {
	url := strings.TrimRight(p.cfg.MemoryServiceHTTP, "/") + "/memory/latest?session_id=" + sessionID
	req, _ := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	resp, err := p.httpClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		b, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("memory/latest: %s", string(b))
	}
	var payload struct {
		Messages []map[string]any `json:"messages"`
	}
	_ = json.NewDecoder(resp.Body).Decode(&payload)
	return payload.Messages, nil
}

func (p *Planner) storeSessionDelta(ctx context.Context, sessionID, userPrompt, assistantText string) error {
	url := strings.TrimRight(p.cfg.MemoryServiceHTTP, "/") + "/memory/store"
	body := map[string]any{
		"session_id": sessionID,
		"history": []map[string]any{
			{"role": "user", "content": userPrompt},
			{"role": "assistant", "content": assistantText},
		},
		"prompt":       userPrompt,
		"llm_response": map[string]any{"text": assistantText},
	}
	b, _ := json.Marshal(body)
	req, _ := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	resp, err := p.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	return nil
}

func (p *Planner) storePlaybook(
	ctx context.Context,
	sessionID string,
	prompt string,
	historySequence []map[string]string,
) error {
	// POST to the Memory Service HTTP API to persist the playbook into Mind-KB.
	// The Memory Service is responsible for converting this into a Chroma document.
	url := strings.TrimRight(p.cfg.MemoryServiceHTTP, "/") + "/memory/playbook"

	// Skip storing trivial 1-step sessions (no tool use), but keep the call-site simple.
	if len(historySequence) < 3 {
		return nil
	}

	payload := map[string]any{
		"session_id":       sessionID,
		"prompt":           prompt,
		"history_sequence": historySequence,
	}
	b, _ := json.Marshal(payload)
	req, _ := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")

	resp, err := p.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		out, _ := io.ReadAll(resp.Body)
		return fmt.Errorf("memory/playbook: %s", string(out))
	}
	return nil
}

func (p *Planner) executeTool(ctx context.Context, toolName string, args map[string]any) (string, error) {
	return p.executeToolGRPC(ctx, toolName, args)
}

func (p *Planner) executeToolGRPC(ctx context.Context, toolName string, args map[string]any) (string, error) {
	if p.toolClient == nil {
		return "", fmt.Errorf("rust sandbox tool client is nil")
	}

	if args == nil {
		args = map[string]any{}
	}

	argsJSON, err := json.Marshal(args)
	if err != nil {
		return "", fmt.Errorf("marshal tool args: %w", err)
	}

	// Default sandbox isolation/resource contract values.
	// These are currently advisory (the Rust sandbox may ignore them), but they
	// future-proof the API for a hardened micro-VM runtime.
	const defaultExecutionEnvironment = "generic-docker"
	const defaultCPULimitMHz int32 = 1000
	const defaultMemoryLimitMB int32 = 512
	const defaultTimeoutSeconds int32 = 30

	resp, err := p.toolClient.ExecuteTool(ctx, &pb.ToolRequest{
		ToolName:             toolName,
		ArgsJson:             string(argsJSON),
		ExecutionEnvironment: defaultExecutionEnvironment,
		CpuLimitMhz:          defaultCPULimitMHz,
		MemoryLimitMb:        defaultMemoryLimitMB,
		TimeoutSeconds:       defaultTimeoutSeconds,
	})
	if err != nil {
		return "", fmt.Errorf("ExecuteTool(%q): %w", toolName, err)
	}

	// Keep the tool output structured (LLM-friendly) and consistent across tools.
	out := map[string]any{
		"status": resp.GetStatus(),
		"stdout": resp.GetStdout(),
		"stderr": resp.GetStderr(),
	}
	encoded, _ := json.Marshal(out)
	return string(encoded), nil
}
