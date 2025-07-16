# --- этап сборки -------------------------------------------------
    FROM rust:1.82-slim AS builder
    WORKDIR /app
    
    RUN apt-get update && apt-get install -y --no-install-recommends \
            pkg-config libssl-dev ca-certificates \
        && rm -rf /var/lib/apt/lists/*
    
    # 1. манифесты
    COPY Cargo.toml Cargo.lock ./
    
    # 2. затычка-файл, чтобы Cargo не ругался
    RUN mkdir -p src && echo "fn main() {}" > src/main.rs
    
    # 3. просто скачиваем/компилируем зависимости (быстро закэшируется)
    RUN cargo fetch --locked
    
    # 4. теперь кладём настоящие исходники и ПЕРЕсобираем
    COPY src ./src
    COPY keys /app/keys/
    RUN cargo build --release --locked