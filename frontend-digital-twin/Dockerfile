# --- STAGE 1: BUILD ---
FROM node:20-alpine AS builder
WORKDIR /app

# Copy package files
COPY package.json package-lock.json* ./
RUN npm ci

# Copy source code
COPY . .

# Build arguments for environment variables
ARG VITE_WS_URL=ws://localhost:8181/ws/chat
ARG VITE_SSE_URL=http://localhost:8181/v1/telemetry/stream

# Set environment variables for build
ENV VITE_WS_URL=$VITE_WS_URL
ENV VITE_SSE_URL=$VITE_SSE_URL

# Build the application
RUN npm run build

# --- STAGE 2: RUNTIME ---
FROM nginx:alpine
WORKDIR /usr/share/nginx/html

# Remove default nginx static assets
RUN rm -rf ./*

# Copy built assets from builder
COPY --from=builder /app/dist .

# Copy nginx configuration (optional, for SPA routing)
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 3000

CMD ["nginx", "-g", "daemon off;"]
