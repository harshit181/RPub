# Build Stage
FROM --platform=linux/amd64 rust:1.91.1-slim-bookworm as builder

WORKDIR /usr/src/rpub

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build release binary
RUN cargo build --release

# Runtime Stage
FROM --platform=linux/amd64 debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates libssl3 sqlite3 && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/rpub/target/release/rpub /usr/local/bin/rpub

# Copy static assets
COPY static /app/static

# Copy database
COPY rpub.db /app/rpub.db

# Expose port
EXPOSE 3000

# Set environment variables
ENV RUST_LOG=info

# Run the application
CMD ["rpub"]
