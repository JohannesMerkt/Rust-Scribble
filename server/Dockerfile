FROM rust:1.62 as builder
WORKDIR /app
COPY ../ ./
WORKDIR /app/server
RUN cargo build --release

FROM rust:1.62 as runner
WORKDIR /server
COPY --from=builder /app/server/target/release ./

EXPOSE 3000

CMD ["./rust_scribble_server"]