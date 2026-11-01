FROM rust:1.97-slim AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
COPY --from=build /app/target/release/ad-aggregator /usr/local/bin/ad-aggregator
WORKDIR /data
ENTRYPOINT ["ad-aggregator"]
CMD ["--input", "ad_data.csv", "--output", "results"]
