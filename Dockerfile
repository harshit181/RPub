FROM rust:1.92.0-slim-trixie as builder

WORKDIR /usr/src/rsspub

RUN apt-get update && apt-get install -y pkg-config build-essential && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release --features alternative-alloc

FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/rsspub/target/release/rsspub /usr/local/bin/rsspub

COPY static /app/static
RUN mkdir -p /app/db
EXPOSE 3000

ENV RUST_LOG=info,html5ever=error

CMD ["rsspub"]
