# ------------ Сборочный этап ------------
    FROM rust:1.82-slim AS builder
    WORKDIR /app
    
    # 1. ставим зависимости для openssl/ssh
    RUN apt-get update && apt-get install -y --no-install-recommends \
            pkg-config libssl-dev ca-certificates \
        && rm -rf /var/lib/apt/lists/*
    
    # 2. кладём манифесты и заранее скачиваем крейты — кэшируется по Cargo.lock
    COPY Cargo.toml Cargo.lock ./
    RUN cargo fetch --locked
    
    # 3. копируем исходники и собираем уже настоящее приложение
    COPY src ./src
    COPY keys /app/keys/
    RUN cargo build --release --locked
    
    # ------------ Рантайм-этап -------------
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y --no-install-recommends \
            ca-certificates libssl3 libssh2-1 openssh-client \
        && rm -rf /var/lib/apt/lists/*
    
    WORKDIR /app
    COPY --from=builder /app/target/release/wakeonlan_bot /usr/local/bin/wakeonlan_bot
    COPY keys /app/keys/
    COPY diagnostic_wrapper.sh /app/diagnostic_wrapper.sh
    RUN chmod 600 /app/keys/* && chmod +x /app/diagnostic_wrapper.sh
    
    ENV RUST_LOG=debug
    CMD ["/app/diagnostic_wrapper.sh"]