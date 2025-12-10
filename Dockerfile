# Build Stage
FROM --platform=linux/amd64 rust:1.91.1-slim-trixie as builder

WORKDIR /usr/src/rpub

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build / binary
RUN cargo build --release --features alternative-alloc

# Runtime Stage
FROM --platform=linux/amd64 debian:trixie-slim

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
