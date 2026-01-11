package main

import (
	"context"
	"log"
	"os"
	"os/signal"
	"syscall"

	"github.com/go-redis/redis/v8"
)

func getenv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func main() {
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	redisAddr := getenv("REDIS_ADDR", "redis:6379")
	channel := getenv("PAGI_NOTIFICATIONS_CHANNEL", "pagi_notifications")

	rdb := redis.NewClient(&redis.Options{Addr: redisAddr})
	defer func() { _ = rdb.Close() }()

	if err := rdb.Ping(ctx).Err(); err != nil {
		log.Fatalf("failed to connect to redis at %s: %v", redisAddr, err)
	}

	sub := rdb.Subscribe(ctx, channel)
	defer func() { _ = sub.Close() }()

	log.Printf("notification-service subscribed to redis channel=%s addr=%s", channel, redisAddr)

	quit := make(chan os.Signal, 1)
	signal.Notify(quit, os.Interrupt, syscall.SIGTERM)

	msgCh := sub.Channel()
	for {
		select {
		case <-quit:
			log.Println("notification-service shutting down")
			return
		case msg, ok := <-msgCh:
			if !ok {
				log.Println("redis subscription channel closed")
				return
			}
			// Payload is JSON published by the Agent Planner.
			log.Printf("notification: %s", msg.Payload)
		}
	}
}
