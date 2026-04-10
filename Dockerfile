# ──────────────────────────────────────────
# Stage 1: Build Frontend
# ──────────────────────────────────────────
FROM node:20-alpine AS frontend-builder

WORKDIR /app/frontend

COPY frontend/package*.json ./
RUN npm ci

COPY frontend/ ./
RUN npm run build

# ──────────────────────────────────────────
# Stage 2: Build Backend
# ──────────────────────────────────────────
FROM rust:1.90-alpine AS backend-builder

RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static

WORKDIR /app

COPY backend/Cargo.toml backend/Cargo.lock ./

ENV SQLX_OFFLINE=true
# Статично лінкуємо тільки openssl, не весь CRT
# +crt-static прибрано — воно ламає proc-macro на musl
ENV OPENSSL_STATIC=true

RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

COPY backend/src ./src
COPY backend/.sqlx ./.sqlx
COPY backend/migrations ./migrations

RUN touch src/main.rs && cargo build --release

# ──────────────────────────────────────────
# Stage 3: Runtime
# ──────────────────────────────────────────
FROM alpine:3.20 AS runtime

RUN apk add --no-cache ca-certificates

WORKDIR /app

COPY --from=backend-builder /app/target/release/AiRecipeSearch ./AiRecipeSearch
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist

EXPOSE 8080
ENV RUST_LOG=info
ENV FRONTEND_DIST=./frontend/dist

ENTRYPOINT ["./AiRecipeSearch"]