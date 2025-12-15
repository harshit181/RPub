FROM rust:1.92.0-slim-trixie as builder

WORKDIR /usr/src/rsspub

RUN apt-get update && apt-get install -y pkg-config build-essential && rm -rf /var/lib/apt/lists/*

COPY . .

RUN cargo build --release --features alternative-alloc

FROM denoland/deno:alpine as ui-builder

WORKDIR /project

COPY ui/package.json ui/deno.lock* ./
RUN deno install

COPY ui .
RUN deno task build

FROM debian:trixie-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/rsspub/target/release/rsspub /usr/local/bin/rsspub

COPY --from=ui-builder /static /app/static
COPY static/cover.jpg /app/static/cover.jpg
RUN mkdir -p /app/static/epubs
RUN mkdir -p /app/db
EXPOSE 3000

ENV RUST_LOG=info,html5ever=error

CMD ["rsspub"]
