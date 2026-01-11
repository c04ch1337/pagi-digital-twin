package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"time"

	"github.com/gin-gonic/gin"
	"github.com/google/uuid"
)

const SERVICE_NAME = "backend-go-bff"
const VERSION = "1.0.0"
const DEFAULT_TIMEOUT_SECONDS = 2
const DEFAULT_BFF_PORT = 8002

// --- Config and Environment ---
type Config struct {
	PyAgentURL     string
	RustSandboxURL string
	MemoryURL      string
	Timeout        time.Duration
	Port           int
}

// Function to load config from environment
func loadConfig() Config {
	timeoutSeconds, _ := strconv.Atoi(os.Getenv("REQUEST_TIMEOUT_SECONDS"))
	if timeoutSeconds == 0 {
		timeoutSeconds = DEFAULT_TIMEOUT_SECONDS
	}

	port, _ := strconv.Atoi(os.Getenv("GO_BFF_PORT"))
	if port == 0 {
		port = DEFAULT_BFF_PORT
	}

	pyAgentURL := os.Getenv("PY_AGENT_URL")
	if pyAgentURL == "" {
		pyAgentURL = "http://localhost:8000"
	}

	rustSandboxURL := os.Getenv("RUST_SANDBOX_URL")
	if rustSandboxURL == "" {
		rustSandboxURL = "http://localhost:8001"
	}

	memoryURL := os.Getenv("MEMORY_URL")
	if memoryURL == "" {
		memoryURL = "http://localhost:8003"
	}

	return Config{
		PyAgentURL:     pyAgentURL,
		RustSandboxURL: rustSandboxURL,
		MemoryURL:      memoryURL,
		Timeout:        time.Duration(timeoutSeconds) * time.Second,
		Port:           port,
	}
}

// --- Structured Logging ---
func logJSON(level string, message string, fields map[string]interface{}) {
	logEntry := map[string]interface{}{
		"timestamp": time.Now().Format(time.RFC3339Nano),
		"level":     level,
		"service":   SERVICE_NAME,
		"message":   message,
	}
	for k, v := range fields {
		logEntry[k] = v
	}
	data, _ := json.Marshal(logEntry)
	fmt.Println(string(data))
}

// --- Handlers ---

// GET /health
func healthCheck(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"service": SERVICE_NAME,
		"status":  "ok",
		"version": VERSION,
	})
}

// POST /api/v1/echo - Safe wiring confirmation (MUST NOT call downstream)
func echoHandler(c *gin.Context) {
	var body map[string]interface{}
	_ = c.BindJSON(&body)

	// Use X-Request-Id from header or body, or generate a new one
	requestID := c.GetHeader("X-Request-Id")
	if requestID == "" {
		if v, ok := body["request_id"].(string); ok && v != "" {
			requestID = v
		} else {
			requestID = uuid.New().String()
		}
	}

	receivedFrom := body["ping"]
	if receivedFrom == nil {
		receivedFrom = "unknown"
	}

	logJSON("info", "Received echo request", map[string]interface{}{
		"request_id":      requestID,
		"received_from":   receivedFrom,
		"received_fields": body,
	})

	c.JSON(http.StatusOK, gin.H{
		"service":    SERVICE_NAME,
		"received":   body,
		"request_id": requestID,
	})
}

type fetchResult struct {
	name string
	data interface{}
	err  error
}

// Internal function to concurrently fetch data from downstream service
func concurrentFetch(ctx context.Context, client *http.Client, method, url, name, requestID string, body io.Reader, ch chan<- fetchResult) {
	req, err := http.NewRequestWithContext(ctx, method, url, body)
	if err != nil {
		ch <- fetchResult{name: name, err: fmt.Errorf("request creation failed: %w", err)}
		return
	}

	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Request-Id", requestID)

	resp, err := client.Do(req)
	if err != nil {
		ch <- fetchResult{name: name, err: fmt.Errorf("network error: %w", err)}
		return
	}
	defer resp.Body.Close()

	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		ch <- fetchResult{name: name, err: fmt.Errorf("failed to read response body: %w", err)}
		return
	}

	if resp.StatusCode != http.StatusOK {
		ch <- fetchResult{name: name, err: fmt.Errorf("status code %d: %s", resp.StatusCode, string(bodyBytes))}
		return
	}

	var data interface{}
	if err := json.Unmarshal(bodyBytes, &data); err != nil {
		ch <- fetchResult{name: name, data: string(bodyBytes), err: nil} // Raw data if unmarshal fails
		return
	}

	ch <- fetchResult{name: name, data: data, err: nil}
}

// GET /api/v1/agi/dashboard-data
func dashboardDataHandler(cfg Config) gin.HandlerFunc {
	return func(c *gin.Context) {
		startTime := time.Now()

		requestID := c.GetHeader("X-Request-Id")
		if requestID == "" {
			requestID = uuid.New().String()
		}

		logJSON("info", "Starting dashboard aggregation", map[string]interface{}{"request_id": requestID})

		// Use a context with timeout for all downstream calls
		ctx, cancel := context.WithTimeout(c.Request.Context(), cfg.Timeout)
		defer cancel()

		client := &http.Client{Timeout: cfg.Timeout}
		ch := make(chan fetchResult, 3)

		// 1. Python Agent (POST)
		go concurrentFetch(ctx, client, "POST", cfg.PyAgentURL+"/api/v1/plan", "python_agent", requestID, bytes.NewBufferString("{}"), ch)

		// 2. Rust Sandbox (POST)
		rustBody := bytes.NewBufferString(`{"tool_name": "demo"}`)
		go concurrentFetch(ctx, client, "POST", cfg.RustSandboxURL+"/api/v1/execute_tool", "rust_sandbox", requestID, rustBody, ch)

		// 3. Mock Memory (GET)
		go concurrentFetch(ctx, client, "GET", cfg.MemoryURL+"/memory/latest", "memory", requestID, nil, ch)

		results := make(map[string]interface{})

		for i := 0; i < 3; i++ {
			result := <-ch
			if result.err != nil {
				results[result.name] = map[string]interface{}{"error": result.err.Error(), "status": "failed"}
			} else {
				results[result.name] = result.data
			}
		}

		elapsed := time.Since(startTime)
		logJSON("info", "Dashboard aggregation complete", map[string]interface{}{
			"request_id": requestID,
			"latency_ms": elapsed.Milliseconds(),
		})

		c.JSON(http.StatusOK, gin.H{
			"service":    SERVICE_NAME,
			"status":     "ok",
			"request_id": requestID,
			"data":       results,
		})
	}
}

func main() {
	cfg := loadConfig()

	// Configure Gin for structured logging (optional, as we use a custom logger here)
	gin.SetMode(gin.ReleaseMode)

	router := gin.New()
	router.Use(gin.Recovery())
	router.Use(func(c *gin.Context) {
		// Log request details via custom logger
		startTime := time.Now()
		c.Next()
		latency := time.Since(startTime)

		requestID := c.GetHeader("X-Request-Id")
		if requestID == "" {
			requestID = uuid.New().String()
		}

		logJSON("info", "Request processed", map[string]interface{}{
			"request_id":  requestID,
			"method":      c.Request.Method,
			"path":        c.Request.URL.Path,
			"status":      c.Writer.Status(),
			"latency_ms":  latency.Milliseconds(),
			"remote_addr": c.ClientIP(),
		})
	})

	router.GET("/health", healthCheck)
	router.POST("/api/v1/echo", echoHandler)
	router.GET("/api/v1/agi/dashboard-data", dashboardDataHandler(cfg))

	logJSON("info", "Starting server", map[string]interface{}{"port": cfg.Port, "version": VERSION})
	if err := router.Run(fmt.Sprintf(":%d", cfg.Port)); err != nil {
		logJSON("fatal", "Failed to run server", map[string]interface{}{"error": err.Error()})
		os.Exit(1)
	}
}
