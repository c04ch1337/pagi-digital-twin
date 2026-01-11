package main

import (
	"context"
	"crypto/subtle"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"backend-go-agent-planner/agent"
	"backend-go-agent-planner/internal/logger"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/google/uuid"
	"go.opentelemetry.io/contrib/instrumentation/net/http/otelhttp"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracegrpc"
	otelprom "go.opentelemetry.io/otel/exporters/prometheus"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/sdk/metric"
	sdkresource "go.opentelemetry.io/otel/sdk/resource"
	"go.opentelemetry.io/otel/sdk/trace"
	semconv "go.opentelemetry.io/otel/semconv/v1.26.0"

	promclient "github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

func initOpenTelemetry(ctx context.Context) (shutdown func(context.Context) error, promHandler http.Handler, err error) {
	serviceName := os.Getenv("OTEL_SERVICE_NAME")
	if strings.TrimSpace(serviceName) == "" {
		serviceName = "backend-go-agent-planner"
	}

	res, err := sdkresource.Merge(
		sdkresource.Default(),
		sdkresource.NewWithAttributes(
			semconv.SchemaURL,
			semconv.ServiceName(serviceName),
		),
	)
	if err != nil {
		return nil, nil, err
	}

	// --- Tracing (OTLP/gRPC exporter) ---
	otlpEndpoint := os.Getenv("OTEL_EXPORTER_OTLP_ENDPOINT")
	if strings.TrimSpace(otlpEndpoint) == "" {
		otlpEndpoint = "localhost:4317"
	}

	traceExp, err := otlptracegrpc.New(
		ctx,
		otlptracegrpc.WithEndpoint(otlpEndpoint),
		otlptracegrpc.WithDialOption(grpc.WithTransportCredentials(insecure.NewCredentials())),
	)
	if err != nil {
		return nil, nil, err
	}

	tp := trace.NewTracerProvider(
		trace.WithBatcher(traceExp),
		trace.WithResource(res),
	)
	otel.SetTracerProvider(tp)
	otel.SetTextMapPropagator(propagation.NewCompositeTextMapPropagator(propagation.TraceContext{}, propagation.Baggage{}))

	// --- Metrics (Prometheus exporter) ---
	reg := promclient.NewRegistry()
	promExp, err := otelprom.New(otelprom.WithRegisterer(reg))
	if err != nil {
		_ = tp.Shutdown(ctx)
		return nil, nil, err
	}
	mp := metric.NewMeterProvider(
		metric.WithReader(promExp),
		metric.WithResource(res),
	)
	otel.SetMeterProvider(mp)

	shutdown = func(ctx context.Context) error {
		err1 := tp.Shutdown(ctx)
		err2 := mp.Shutdown(ctx)
		if err1 != nil {
			return err1
		}
		return err2
	}

	return shutdown, promhttp.HandlerFor(reg, promhttp.HandlerOpts{}), nil
}

// apiKeyMiddleware validates the X-API-Key header against the configured API key.
// This is a critical security control for production deployments.
// If PAGI_API_KEY is not set, authentication is DISABLED (dev mode only).
func apiKeyMiddleware(next http.Handler) http.Handler {
	apiKey := os.Getenv("PAGI_API_KEY")
	authEnabled := strings.TrimSpace(apiKey) != ""

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		// Skip auth for health checks (required for K8s probes)
		if r.URL.Path == "/health" || r.URL.Path == "/ready" || r.URL.Path == "/live" || r.URL.Path == "/metrics" {
			next.ServeHTTP(w, r)
			return
		}

		// If no API key configured, log warning and allow (dev mode)
		if !authEnabled {
			logger.NewContextLogger(r.Context()).Warn(
				"auth_disabled",
				"path", r.URL.Path,
				"warning", "PAGI_API_KEY not set - authentication disabled (INSECURE)",
			)
			next.ServeHTTP(w, r)
			return
		}

		// Extract API key from header
		providedKey := r.Header.Get("X-API-Key")
		if providedKey == "" {
			// Also check Authorization: Bearer <token>
			authHeader := r.Header.Get("Authorization")
			if strings.HasPrefix(authHeader, "Bearer ") {
				providedKey = strings.TrimPrefix(authHeader, "Bearer ")
			}
		}

		// Constant-time comparison to prevent timing attacks
		if subtle.ConstantTimeCompare([]byte(providedKey), []byte(apiKey)) != 1 {
			logger.NewContextLogger(r.Context()).Warn(
				"auth_failed",
				"path", r.URL.Path,
				"remote_addr", r.RemoteAddr,
			)
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusUnauthorized)
			_ = json.NewEncoder(w).Encode(map[string]string{
				"error":   "unauthorized",
				"message": "Invalid or missing API key",
			})
			return
		}

		next.ServeHTTP(w, r)
	})
}

// traceIDMiddleware generates or extracts a trace ID from the request header
// and adds it to the request context.
func traceIDMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		traceID := r.Header.Get(string(logger.TraceIDKey))
		if traceID == "" {
			traceID = uuid.New().String()
		}

		// Propagate ID in response header for client visibility.
		w.Header().Set(string(logger.TraceIDKey), traceID)

		// Inject ID into context.
		ctx := context.WithValue(r.Context(), logger.TraceIDKey, traceID)
		next.ServeHTTP(w, r.WithContext(ctx))
	})
}

// requestLogMiddleware logs one line per request, always including trace_id when present.
func requestLogMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		ww := middleware.NewWrapResponseWriter(w, r.ProtoMajor)
		next.ServeHTTP(ww, r)
		logger.NewContextLogger(r.Context()).Info(
			"http_request",
			"method", r.Method,
			"path", r.URL.Path,
			"status", ww.Status(),
			"latency_ms", time.Since(start).Milliseconds(),
		)
	})
}

func main() {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	log := logger.NewContextLogger(ctx)

	shutdownOTel, promHandler, err := initOpenTelemetry(ctx)
	if err != nil {
		log.Error("otel_init_failed", "error", err)
		os.Exit(1)
	}
	defer func() { _ = shutdownOTel(context.Background()) }()

	// 1) Initialize Configuration and Planner
	cfg := agent.ConfigFromEnv()
	planner, err := agent.NewPlanner(ctx, cfg)
	if err != nil {
		log.Error("planner_init_failed", "error", err)
		os.Exit(1)
	}
	defer planner.Close()

	// 2) Setup Router with Security Middleware
	r := chi.NewRouter()
	r.Use(middleware.Recoverer)
	r.Use(func(next http.Handler) http.Handler {
		return otelhttp.NewHandler(
			next,
			"http.server",
			otelhttp.WithSpanNameFormatter(func(_ string, r *http.Request) string {
				return r.Method + " " + r.URL.Path
			}),
		)
	})
	r.Use(traceIDMiddleware)
	r.Use(apiKeyMiddleware) // SECURITY: API key authentication
	r.Use(requestLogMiddleware)

	port := os.Getenv("AGENT_PLANNER_PORT")
	if port == "" {
		port = "8080" // Default port, overridden to 8585 by docker-compose
	}

	// Health Check Endpoint
	r.Get("/health", func(w http.ResponseWriter, _r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		_ = json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
	})

	// Prometheus metrics endpoint (OpenTelemetry Prometheus exporter).
	if promHandler != nil {
		r.Handle("/metrics", promHandler)
	}

	// Main Planning/Execution Endpoint
	r.Post("/plan", handlePlan(planner))
	// Backwards/alternate naming: allow either endpoint.
	r.Post("/run", handlePlan(planner))

	// 3) Start Server
	server := &http.Server{
		Addr:    fmt.Sprintf(":%s", port),
		Handler: r,
	}

	go func() {
		log.Info("agent_planner_listening", "port", port)
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Error("http_server_failed", "port", port, "error", err)
			os.Exit(1)
		}
	}()

	// 4) Graceful Shutdown
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, os.Interrupt, syscall.SIGTERM)
	<-quit

	log.Info("server_shutdown_start")
	ctxTimeout, cancelTimeout := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancelTimeout()

	if err := server.Shutdown(ctxTimeout); err != nil {
		log.Error("server_shutdown_forced", "error", err)
		os.Exit(1)
	}
	log.Info("server_shutdown_complete")
}

type PlanRequest struct {
	Prompt    string           `json:"prompt"`
	SessionID string           `json:"session_id"`
	Resources []agent.Resource `json:"resources"`
}

type PlanResponse struct {
	Result string `json:"result"`
}

func writeJSONError(w http.ResponseWriter, status int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"error": msg})
}

func handlePlan(p *agent.Planner) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		log := logger.NewContextLogger(r.Context())

		var req PlanRequest
		if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
			writeJSONError(w, http.StatusBadRequest, "Invalid request body")
			return
		}

		if req.Prompt == "" || req.SessionID == "" {
			writeJSONError(w, http.StatusBadRequest, "Prompt and session_id are required")
			return
		}

		for i, res := range req.Resources {
			if strings.TrimSpace(res.Type) == "" || strings.TrimSpace(res.URI) == "" {
				writeJSONError(w, http.StatusBadRequest, fmt.Sprintf("resources[%d] must include non-empty type and uri", i))
				return
			}
		}

		log.Info("agent_loop_start", "session_id", req.SessionID)
		result, err := p.AgentLoop(r.Context(), req.Prompt, req.SessionID, req.Resources)
		if err != nil {
			log.Error("agent_loop_failed", "session_id", req.SessionID, "error", err)
			writeJSONError(w, http.StatusInternalServerError, fmt.Sprintf("Agent execution failed: %s", err.Error()))
			return
		}
		log.Info("agent_loop_complete", "session_id", req.SessionID)

		resp := PlanResponse{Result: result}
		if err := json.NewEncoder(w).Encode(resp); err != nil {
			log.Error("encode_response_failed", "error", err)
		}
	}
}
