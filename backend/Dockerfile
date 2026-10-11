# Tenor backend: one image, three binaries (api, worker, seed).
#   docker build -t tenor-backend backend/
#   docker run -e DATABASE_URL=... tenor-backend            # api (default)
#   APP_ENV defaults to production (refuses mock data); demo deploys set APP_ENV=staging
#   docker run -e DATABASE_URL=... tenor-backend worker     # background jobs
#   docker run -e DATABASE_URL=... tenor-backend seed       # demo baskets (mock mode)

FROM rust:1-bookworm AS build
WORKDIR /app
# Tolerate slow networks while fetching ~400 crates.
ENV CARGO_NET_RETRY=10 CARGO_HTTP_TIMEOUT=120 CARGO_HTTP_MULTIPLEXING=false
# Cache dependencies: build once with stub sources, then the real ones.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin examples \
 && echo "fn main() {}" > src/bin/api.rs && cp src/bin/api.rs src/bin/worker.rs \
 && cp src/bin/api.rs src/bin/seed.rs && cp src/bin/api.rs examples/smoke.rs \
 && cp src/bin/api.rs examples/seed.rs && echo "" > src/lib.rs \
 && cargo build --release --bin api --bin worker --bin seed \
 && rm -rf src examples
COPY . .
RUN touch src/lib.rs src/bin/*.rs && cargo build --release --bin api --bin worker --bin seed

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* && useradd -r -u 10001 tenor
COPY --from=build /app/target/release/api /app/target/release/worker /app/target/release/seed /usr/local/bin/
COPY --chmod=755 start.sh /usr/local/bin/start
USER tenor
ENV APP_ENV=production RUST_LOG=info
EXPOSE 8080
# Default: the API (seeding first when SEED_ON_START=true). Hosts override
# the start command per service only to run the worker: `start worker`.
CMD ["start", "api"]
