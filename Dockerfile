# === Этап сборки (builder) ===
FROM rust:1.82-slim AS builder
WORKDIR /app

# Необходимые системные библиотеки для компиляции зависимостей ssh/openssl
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ------------------------------------------------------------
# 1. Кладём манифесты и заранее скачиваем/компилируем зависимости
#    Этот слой будет кэшироваться, пока не изменится Cargo.lock
# ------------------------------------------------------------
COPY Cargo.toml Cargo.lock ./
# Временная заглушка, чтобы Cargo не ругался на отсутствие targets
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Загрузка и сборка зависимостей (компилирует только крейты из crates.io)
RUN cargo fetch --locked

# ------------------------------------------------------------
# 2. Копируем реальные исходники проекта и собираем релизный бинарь
# ------------------------------------------------------------
COPY src ./src
COPY keys /app/keys/
RUN cargo build --release --locked

# === Этап рантайма (минимальный образ) ===
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 libssh2-1 openssh-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
# Копируем собранный бинарь
COPY --from=builder /app/target/release/wakeonlan_bot /usr/local/bin/wakeonlan_bot
# Копируем ключи и диагностический скрипт
COPY keys /app/keys/
COPY diagnostic_wrapper.sh /app/diagnostic_wrapper.sh
RUN chmod 600 /app/keys/* && chmod +x /app/diagnostic_wrapper.sh

ENV RUST_LOG=debug

# Точка входа
CMD ["/app/diagnostic_wrapper.sh"]