FROM rust:1-bullseye as builder
WORKDIR /usr/src/
RUN cargo new myapp --vcs none
WORKDIR /usr/src/myapp
COPY Cargo.toml ./
RUN cargo build --release

# 为了充分利用docker的缓存
COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release


FROM debian:bullseye-slim
RUN apt-get update && apt-get install libpq5 -y
COPY --from=builder /usr/src/myapp/target/release/hole-thu /usr/local/bin/hole-thu
COPY Rocket.toml /usr/local/bin/

CMD ["hole-thu"]
