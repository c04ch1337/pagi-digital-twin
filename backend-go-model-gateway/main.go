package main

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"errors"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"backend-go-model-gateway/internal/logger"
	pb "backend-go-model-gateway/proto/proto" // Reference generated code package
	"backend-go-model-gateway/service"

	openai "github.com/sashabaranov/go-openai"
	"go.opentelemetry.io/contrib/instrumentation/google.golang.org/grpc/otelgrpc"
	"google.golang.org/grpc"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/credentials"
	grpc_health_v1 "google.golang.org/grpc/health/grpc_health_v1"
	"google.golang.org/grpc/status"
)

//go:generate protoc --go_out=./proto --go_opt=paths=source_relative --go-grpc_out=./proto --go-grpc_opt=paths=source_relative proto/model.proto

// --- Configuration ---
const DEFAULT_GRPC_PORT = 50051
const DEFAULT_HTTP_PORT = 8005
const SERVICE_NAME = "backend-go-model-gateway"
const VERSION = "1.0.0"

const (
	defaultProvider          = "openrouter"
	defaultOllamaBaseURL     = "http://localhost:11434"
	defaultRequestTimeoutSec = 5
)

// sharedHTTPClient is a single, long-lived HTTP client that provides connection
// pooling and outbound request tracing for all LLM calls.
//
// NOTE: request-level timeouts should be enforced via context deadlines.
var sharedHTTPClient = newSharedHTTPClient()

func newSharedHTTPClient() *http.Client {
	base := &http.Transport{
		Proxy: http.ProxyFromEnvironment,
		DialContext: (&net.Dialer{
			Timeout:   30 * time.Second,
			KeepAlive: 30 * time.Second,
		}).DialContext,
		ForceAttemptHTTP2:     true,
		MaxIdleConns:          100,
		MaxIdleConnsPerHost:   20,
		IdleConnTimeout:       90 * time.Second,
		TLSHandshakeTimeout:   10 * time.Second,
		ExpectContinueTimeout: 1 * time.Second,
	}

	return &http.Client{
		Transport: ClientTraceTransport(base),
	}
}

type llmProvider string

const (
	providerOpenRouter llmProvider = "openrouter"
	providerOllama     llmProvider = "ollama"
	// providerMock is a zero-dependency dev mode that returns deterministic JSON
	// plans (and optionally tool calls) without contacting any external LLM.
	providerMock llmProvider = "mock"
)

type llmRuntime struct {
	Provider llmProvider
	Model    string
	Client   *openai.Client
}

// noopRAGClient is a fallback RAG client used when the Memory Service is not
// reachable during boot (common in bare-metal dev when services start in
// parallel). It keeps the model gateway online and simply returns no RAG
// context.
type noopRAGClient struct{}

func (noopRAGClient) GetContext(_ context.Context, _ VectorQueryRequest) ([]VectorQueryMatch, error) {
	return []VectorQueryMatch{}, nil
}

// --- Tool Definitions (for LLM tool-use prompting) ---
type ToolDefinition struct {
	Name        string               `json:"name"`
	Description string               `json:"description"`
	Parameters  map[string]ToolParam `json:"parameters"`
}

type ToolParam struct {
	Type        string `json:"type"`
	Description string `json:"description"`
}

var availableTools = []ToolDefinition{
	{
		Name:        "web_search",
		Description: "Use this tool to find up-to-date information or external knowledge.",
		Parameters: map[string]ToolParam{
			"query": {Type: "string", Description: "The search query."},
		},
	},
}

func getEnv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func getEnvInt(key string, fallback int) int {
	v := os.Getenv(key)
	if v == "" {
		return fallback
	}
	i, err := strconv.Atoi(v)
	if err != nil || i <= 0 {
		return fallback
	}
	return i
}

func normalizeOllamaBaseURL(base string) string {
	// Ollama's OpenAI-compatible endpoint is typically at /v1
	base = strings.TrimRight(base, "/")
	if strings.HasSuffix(base, "/v1") {
		return base
	}
	return base + "/v1"
}

func loadMTLSServerCreds() (credentials.TransportCredentials, bool, error) {
	serverCertPath := os.Getenv("TLS_SERVER_CERT_PATH")
	serverKeyPath := os.Getenv("TLS_SERVER_KEY_PATH")
	caCertPath := os.Getenv("TLS_CA_CERT_PATH")

	// Allow non-TLS local dev unless explicitly configured.
	if serverCertPath == "" && serverKeyPath == "" && caCertPath == "" {
		return nil, false, nil
	}
	if serverCertPath == "" || serverKeyPath == "" || caCertPath == "" {
		return nil, false, fmt.Errorf("mTLS misconfigured: TLS_SERVER_CERT_PATH, TLS_SERVER_KEY_PATH, TLS_CA_CERT_PATH must all be set")
	}

	serverCert, err := tls.LoadX509KeyPair(serverCertPath, serverKeyPath)
	if err != nil {
		return nil, false, fmt.Errorf("load server keypair (%s, %s): %w", filepath.Clean(serverCertPath), filepath.Clean(serverKeyPath), err)
	}

	caPEM, err := os.ReadFile(caCertPath)
	if err != nil {
		return nil, false, fmt.Errorf("read CA cert (%s): %w", filepath.Clean(caCertPath), err)
	}
	caPool := x509.NewCertPool()
	if ok := caPool.AppendCertsFromPEM(caPEM); !ok {
		return nil, false, fmt.Errorf("append CA certs from PEM (%s): no certs parsed", filepath.Clean(caCertPath))
	}

	conf := &tls.Config{
		MinVersion:   tls.VersionTLS12,
		Certificates: []tls.Certificate{serverCert},
		ClientCAs:    caPool,
		ClientAuth:   tls.RequireAndVerifyClientCert,
		NextProtos:   []string{"h2"},
	}

	return credentials.NewTLS(conf), true, nil
}

func initializeLLMClient() (*llmRuntime, error) {
	provider := llmProvider(strings.ToLower(getEnv("LLM_PROVIDER", defaultProvider)))

	// Zero-dependency local/dev mode.
	if provider == providerMock {
		return &llmRuntime{Provider: providerMock, Model: "mock", Client: nil}, nil
	}

	// Shared OpenAI-compatible client setup (go-openai)
	switch provider {
	case providerOllama:
		ollamaBase := normalizeOllamaBaseURL(getEnv("OLLAMA_BASE_URL", defaultOllamaBaseURL))
		model := getEnv("OLLAMA_MODEL_NAME", "llama3")
		cfg := openai.DefaultConfig("")
		cfg.BaseURL = ollamaBase
		cfg.HTTPClient = sharedHTTPClient
		client := openai.NewClientWithConfig(cfg)
		return &llmRuntime{Provider: providerOllama, Model: model, Client: client}, nil

	case providerOpenRouter, "":
		apiKey := os.Getenv("OPENROUTER_API_KEY")
		if apiKey == "" {
			return nil, fmt.Errorf("OPENROUTER_API_KEY is required when LLM_PROVIDER=openrouter")
		}
		model := getEnv("OPENROUTER_MODEL_NAME", "mistralai/mistral-7b-instruct:free")
		cfg := openai.DefaultConfig(apiKey)
		cfg.BaseURL = "https://openrouter.ai/api/v1"
		cfg.HTTPClient = sharedHTTPClient
		client := openai.NewClientWithConfig(cfg)
		return &llmRuntime{Provider: providerOpenRouter, Model: model, Client: client}, nil

	default:
		return nil, fmt.Errorf("unsupported LLM_PROVIDER=%q (supported: openrouter, ollama, mock)", provider)
	}
}

// --- gRPC Server Implementation ---
type server struct {
	pb.UnimplementedModelGatewayServer
	llm *llmRuntime
	// vectorDB provides Retrieval-Augmented Generation (RAG) context for prompts.
	vectorDB RAGContextClient
	// Per-request timeout for the LLM call.
	requestTimeout time.Duration
}

func buildMockPlanResponse(in *pb.PlanRequest, requestStart time.Time) *pb.PlanResponse {
	// Zero-dependency mock provider: return deterministic strict JSON.
	// This keeps the stack usable out-of-the-box without any API keys and also
	// serves as a resilience fallback when upstream LLM providers rate-limit.
	prompt := strings.TrimSpace(in.GetPrompt())
	lower := strings.ToLower(prompt)

	// Heuristic: if the user asks for “latest” / “search” / “web”, emit a tool call.
	if strings.Contains(lower, "search") || strings.Contains(lower, "web") || strings.Contains(lower, "latest") {
		payload := map[string]any{
			"model_type": string(providerMock),
			"prompt":     in.GetPrompt(),
			"tool": map[string]any{
				"name": "web_search",
				"args": map[string]any{"query": prompt},
			},
		}
		b, _ := json.Marshal(payload)
		return &pb.PlanResponse{Plan: string(b), ModelName: "mock", LatencyMs: time.Since(requestStart).Milliseconds()}
	}

	steps := []string{
		"Restate the objective in one sentence and identify constraints.",
		"Propose a minimal 3-step plan with clear inputs/outputs.",
		"Return the plan as strict JSON for downstream parsing.",
	}
	payload := map[string]any{
		"model_type": string(providerMock),
		"prompt":     in.GetPrompt(),
		"steps":      steps,
	}
	b, _ := json.Marshal(payload)
	return &pb.PlanResponse{Plan: string(b), ModelName: "mock", LatencyMs: time.Since(requestStart).Milliseconds()}
}

// healthServer implements the standard gRPC Health Checking Protocol.
//
// The goal is to report NOT_SERVING if critical downstream dependencies are
// unavailable so orchestrators (Docker/K8s) avoid sending traffic prematurely.
type healthServer struct {
	grpc_health_v1.UnimplementedHealthServer

	llm       *llmRuntime
	ragClient *RAGGRPCClient
}

func (h *healthServer) Check(ctx context.Context, _ *grpc_health_v1.HealthCheckRequest) (*grpc_health_v1.HealthCheckResponse, error) {
	// Mock mode is always "serving" (no downstream dependencies).
	if h.llm != nil && h.llm.Provider == providerMock {
		return &grpc_health_v1.HealthCheckResponse{Status: grpc_health_v1.HealthCheckResponse_SERVING}, nil
	}

	// 1) LLM client must be initialized.
	if h.llm == nil || h.llm.Client == nil {
		return &grpc_health_v1.HealthCheckResponse{Status: grpc_health_v1.HealthCheckResponse_NOT_SERVING}, nil
	}

	// 2) Memory Service (RAG) should be reachable (best-effort).
	// If the memory service exports gRPC health, probe it.
	if h.ragClient != nil && h.ragClient.conn != nil {
		probeCtx, cancel := context.WithTimeout(ctx, 2*time.Second)
		defer cancel()
		hc := grpc_health_v1.NewHealthClient(h.ragClient.conn)
		resp, err := hc.Check(probeCtx, &grpc_health_v1.HealthCheckRequest{Service: ""})
		if err != nil || resp.GetStatus() != grpc_health_v1.HealthCheckResponse_SERVING {
			return &grpc_health_v1.HealthCheckResponse{Status: grpc_health_v1.HealthCheckResponse_NOT_SERVING}, nil
		}
	}

	return &grpc_health_v1.HealthCheckResponse{Status: grpc_health_v1.HealthCheckResponse_SERVING}, nil
}

func (h *healthServer) Watch(_ *grpc_health_v1.HealthCheckRequest, _ grpc_health_v1.Health_WatchServer) error {
	return status.Error(codes.Unimplemented, "Watch is not implemented")
}

// GetPlan implements modelgateway.ModelGatewayServer.
func (s *server) GetPlan(ctx context.Context, in *pb.PlanRequest) (*pb.PlanResponse, error) {
	requestStart := time.Now()

	ctx = service.ContextWithTraceIDFromIncomingGRPC(ctx)

	// Bound the LLM call.
	callCtx, cancel := context.WithTimeout(ctx, s.requestTimeout)
	defer cancel()

	provider := "uninitialized"
	model := "uninitialized"
	if s.llm != nil {
		provider = string(s.llm.Provider)
		model = s.llm.Model
	}

	lg := logger.NewContextLogger(callCtx)
	resourceTypes := make([]string, 0, len(in.GetResources()))
	for _, r := range in.GetResources() {
		if r == nil {
			continue
		}
		resourceTypes = append(resourceTypes, r.GetType())
	}
	lg.Info(
		"GetPlan",
		"provider", provider,
		"model", model,
		"prompt", in.GetPrompt(),
		"resource_count", len(in.GetResources()),
		"resource_types", resourceTypes,
	)

	if s.llm == nil {
		return nil, fmt.Errorf("LLM runtime not initialized")
	}

	// Zero-dependency mock provider: return deterministic strict JSON.
	// This keeps docker-compose usable out-of-the-box without any API keys.
	if s.llm.Provider == providerMock {
		return buildMockPlanResponse(in, requestStart), nil
	}

	if s.llm.Client == nil {
		return nil, fmt.Errorf("LLM client not initialized")
	}

	// --- RAG: Retrieve vector context (best-effort; do not fail the request) ---
	// Default top-k for retrieval; the mock currently returns 2 deterministic items regardless.
	const topK = 3
	retrievalPreamble := ""
	if s.vectorDB != nil {
		retrievalStart := time.Now()
		// Temporary stand-in for a future protobuf field: request all conceptual RAG KBs.
		kbList := []string{"Domain-KB", "Body-KB", "Soul-KB"}
		matches, err := s.vectorDB.GetContext(callCtx, VectorQueryRequest{QueryText: in.GetPrompt(), TopK: topK, KnowledgeBases: kbList})
		if err != nil {
			lg.Warn("vector_retrieval_failed", "error", err)
		} else if len(matches) > 0 {
			var contextBuilder strings.Builder
			contextBuilder.WriteString("The following information is retrieved from the knowledge base:\n")
			contextBuilder.WriteString("<context>\n")
			for _, match := range matches {
				// Visually separate KBs in the prompt.
				contextBuilder.WriteString(fmt.Sprintf("**%s**\n", match.KnowledgeBase))
				contextBuilder.WriteString(fmt.Sprintf("ID: %s\nText: %s\n---\n", match.ID, match.Text))
			}
			contextBuilder.WriteString("</context>\n\n")
			retrievalPreamble = contextBuilder.String()

			lg.Info("vector_retrieval_complete", "match_count", len(matches), "latency_ms", time.Since(retrievalStart).Milliseconds())
		}
	}

	// --- Tool schema + strict output instructions ---
	toolsBlob, _ := json.MarshalIndent(availableTools, "", "  ")
	toolsSection := fmt.Sprintf("<available_tools>\n%s\n</available_tools>\n\n", string(toolsBlob))

	// Prompt the model to return strict JSON so downstream can parse either a plan or a tool call.
	system := "" +
		"You are a planning assistant.\n" +
		"Return STRICT JSON only (no markdown, no prose, no code fences).\n\n" +
		"TOOL USE:\n" +
		"- If a tool is necessary, return a STRICT JSON object containing the key 'tool'.\n" +
		"- The 'tool' object MUST have keys: 'name' (string) and 'args' (object).\n" +
		"- Example: {\"tool\":{\"name\":\"web_search\",\"args\":{\"query\":\"...\"}}}\n" +
		"\n" +
		"PLANNING (no tool needed):\n" +
		"- Return a STRICT JSON object containing: 'steps' (array of strings).\n" +
		"\n" +
		toolsSection

	user := retrievalPreamble + fmt.Sprintf("User prompt: %s", in.GetPrompt())

	resp, err := s.llm.Client.CreateChatCompletion(
		callCtx,
		openai.ChatCompletionRequest{
			Model: s.llm.Model,
			Messages: []openai.ChatCompletionMessage{
				{Role: openai.ChatMessageRoleSystem, Content: system},
				{Role: openai.ChatMessageRoleUser, Content: user},
			},
			Temperature: 0.2,
		},
	)
	if err != nil {
		// Resilience: if OpenRouter is rate-limited upstream (429), fall back to the
		// deterministic mock response so the system remains usable.
		if s.llm.Provider == providerOpenRouter {
			var apiErr *openai.APIError
			if errors.As(err, &apiErr) && apiErr.HTTPStatusCode == http.StatusTooManyRequests {
				lg.Warn("llm_rate_limited_falling_back_to_mock", "provider", provider, "model", model, "error", err)
				return buildMockPlanResponse(in, requestStart), nil
			}
		}
		return nil, err
	}

	content := ""
	if len(resp.Choices) > 0 {
		content = resp.Choices[0].Message.Content
	}

	trimmed := strings.TrimSpace(content)

	// Normalize common LLM output formats into strict JSON:
	// - raw JSON object
	// - fenced code block containing JSON
	// - non-JSON text (fallback wrapper)
	stripFences := func(s string) string {
		s = strings.TrimSpace(s)
		if !strings.HasPrefix(s, "```") {
			return s
		}
		// Drop the first fence line
		if idx := strings.Index(s, "\n"); idx >= 0 {
			s = s[idx+1:]
		}
		// Drop the trailing fence
		if end := strings.LastIndex(s, "```"); end >= 0 {
			s = s[:end]
		}
		return strings.TrimSpace(s)
	}

	normalizeJSON := func(raw string) (string, bool) {
		candidate := strings.TrimSpace(raw)
		if !strings.HasPrefix(candidate, "{") {
			return "", false
		}

		var obj map[string]any
		if err := json.Unmarshal([]byte(candidate), &obj); err != nil {
			return "", false
		}

		// Tool-call path: pass through (but ensure tracing fields exist).
		if toolObj, ok := obj["tool"].(map[string]any); ok {
			name, _ := toolObj["name"].(string)
			if strings.TrimSpace(name) == "" {
				return "", false
			}
			if _, ok := toolObj["args"]; !ok {
				toolObj["args"] = map[string]any{}
			}
			if _, ok := obj["model_type"]; !ok {
				obj["model_type"] = provider
			}
			if _, ok := obj["prompt"]; !ok {
				obj["prompt"] = in.GetPrompt()
			}
			b, _ := json.Marshal(obj)
			return string(b), true
		}

		// Planning path: require a non-empty steps array.
		stepsAny, ok := obj["steps"].([]any)
		if !ok || len(stepsAny) == 0 {
			return "", false
		}
		steps := make([]string, 0, len(stepsAny))
		for _, v := range stepsAny {
			if s, ok := v.(string); ok && strings.TrimSpace(s) != "" {
				steps = append(steps, s)
			}
		}
		if len(steps) == 0 {
			return "", false
		}
		payload := map[string]any{
			"model_type": provider,
			"steps":      steps,
			"prompt":     in.GetPrompt(),
		}
		b, _ := json.Marshal(payload)
		return string(b), true
	}

	// 1) Try raw JSON
	if normalized, ok := normalizeJSON(trimmed); ok {
		trimmed = normalized
	} else {
		// 2) Try fenced JSON
		fenced := stripFences(trimmed)
		if normalized, ok := normalizeJSON(fenced); ok {
			trimmed = normalized
		} else {
			// 3) Fallback wrapper
			fallback := map[string]any{
				"model_type": provider,
				"steps":      []string{trimmed},
				"prompt":     in.GetPrompt(),
			}
			b, _ := json.Marshal(fallback)
			trimmed = string(b)
		}
	}

	latencyMs := time.Since(requestStart).Milliseconds()
	return &pb.PlanResponse{
		Plan:      trimmed,
		ModelName: s.llm.Model,
		LatencyMs: latencyMs,
	}, nil
}

func main() {
	// --- OpenTelemetry tracing (best-effort) ---
	if tp, err := InitTracer(context.Background()); err != nil {
		log.Printf(
			`{"timestamp":"%s","level":"warn","service":"%s","component":"tracing","error":%q,"message":"failed to initialize OpenTelemetry; continuing without tracing"}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err.Error(),
		)
	} else {
		defer func() { _ = tp.Shutdown(context.Background()) }()
	}

	// Parse port from environment or flag
	grpcPortEnv := os.Getenv("MODEL_GATEWAY_GRPC_PORT")
	port, err := strconv.Atoi(grpcPortEnv)
	if err != nil || port == 0 {
		port = DEFAULT_GRPC_PORT
	}

	// Initialize Vector DB (RAG) client.
	//
	// In bare-metal dev mode the Memory Service may not be ready when the Model
	// Gateway starts. Don't fail fast here; fall back to a no-op RAG client so the
	// gateway can still serve mock LLM responses and become healthy.
	var ragClient *RAGGRPCClient
	var vectorClient RAGContextClient = noopRAGClient{}

	rigCtx, cancelRAGDial := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancelRAGDial()
	if rc, err := NewRAGGRPCClient(rigCtx); err != nil {
		log.Printf(
			`{"timestamp":"%s","level":"warn","service":"%s","component":"RAGGRPCClient","error":%q,"message":"failed to connect to memory service for RAG; starting with noop RAG client"}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err.Error(),
		)
	} else {
		ragClient = rc
		vectorClient = rc
		defer func() { _ = rc.Close() }()
	}

	// Temporary HTTP endpoint for independent testing of vector retrieval.
	httpPort := getEnvInt("MODEL_GATEWAY_HTTP_PORT", DEFAULT_HTTP_PORT)
	go func() {
		srv := &http.Server{Addr: fmt.Sprintf(":%d", httpPort), Handler: NewHTTPMux(vectorClient)}
		log.Printf(
			`{"timestamp":"%s","level":"info","service":"%s","version":"%s","port":%d,"message":"HTTP server listening (temporary vector-test endpoint)."}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, VERSION, httpPort,
		)
		if err := srv.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
			log.Printf(
				`{"timestamp":"%s","level":"error","service":"%s","error":"http server failed: %v"}`,
				time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err,
			)
		}
	}()

	lis, err := net.Listen("tcp", fmt.Sprintf(":%d", port))
	if err != nil {
		log.Fatalf(
			`{"timestamp": "%s", "level": "fatal", "service": "%s", "error": "failed to listen: %v"}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err,
		)
	}

	llm, err := initializeLLMClient()
	if err != nil {
		log.Fatalf(
			`{"timestamp": "%s", "level": "fatal", "service": "%s", "error": %q}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err.Error(),
		)
	}

	timeoutSec := getEnvInt("REQUEST_TIMEOUT_SECONDS", defaultRequestTimeoutSec)

	serverOpts := []grpc.ServerOption{grpc.StatsHandler(otelgrpc.NewServerHandler())}
	if creds, enabled, err := loadMTLSServerCreds(); err != nil {
		log.Fatalf(
			`{"timestamp": "%s", "level": "fatal", "service": "%s", "error": %q}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err.Error(),
		)
	} else if enabled {
		serverOpts = append(serverOpts, grpc.Creds(creds))
		log.Printf(
			`{"timestamp": "%s", "level": "info", "service": "%s", "message": "mTLS enabled for gRPC server."}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME,
		)
	} else {
		log.Printf(
			`{"timestamp": "%s", "level": "warn", "service": "%s", "message": "mTLS NOT enabled for gRPC server (TLS_* env vars not set); running insecure."}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME,
		)
	}

	s := grpc.NewServer(serverOpts...)
	grpc_health_v1.RegisterHealthServer(s, &healthServer{llm: llm, ragClient: ragClient})
	pb.RegisterModelGatewayServer(s, &server{llm: llm, vectorDB: vectorClient, requestTimeout: time.Duration(timeoutSec) * time.Second})

	log.Printf(
		`{"timestamp": "%s", "level": "info", "service": "%s", "version": "%s", "port": %d, "provider": %q, "model": %q, "message": "gRPC server listening."}`,
		time.Now().Format(time.RFC3339Nano), SERVICE_NAME, VERSION, port, llm.Provider, llm.Model,
	)

	if err := s.Serve(lis); err != nil {
		log.Fatalf(
			`{"timestamp": "%s", "level": "fatal", "service": "%s", "error": "failed to serve: %v"}`,
			time.Now().Format(time.RFC3339Nano), SERVICE_NAME, err,
		)
	}
}
