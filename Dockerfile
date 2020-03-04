FROM rust:1.41.1 as builder
WORKDIR /usr/src/ketera-bot
COPY . .
RUN cargo install --path .

FROM debian:buster-slim
WORKDIR /root
RUN apt-get update && apt-get install -y ca-certificates libssl-dev && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/ketera-bot /usr/local/bin/ketera-bot
CMD ["ketera-bot"]