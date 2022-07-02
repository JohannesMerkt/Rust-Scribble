FROM rust:1.62.0-slim-buster as builder
WORKDIR /app
COPY ../ ./
WORKDIR /app/server
RUN cargo build --release

FROM rust:1.62.0-slim-buster as runner
COPY --from=builder /app/server/target/release/rust_scribble_server .

EXPOSE 3000

CMD ["./rust_scribble_server"]