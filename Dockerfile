# === Сборочный этап ===
FROM rust:1.71-slim AS builder
WORKDIR /app

# Копируем манифест и зависимости
COPY Cargo.toml ./
# Опционально, ускоряем кэш
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --locked

# Копируем исходники проекта
COPY src ./src
COPY keys /app/keys/
RUN cargo build --release --locked

# === Финальный образ ===
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libssh2-1 openssh-client && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/wakeonlan_bot /usr/local/bin/wakeonlan_bot
COPY keys /app/keys/
RUN chmod 600 /app/keys/*

ENV RUST_LOG=info

CMD ["wakeonlan_bot"] 