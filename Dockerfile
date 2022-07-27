FROM rust:1.62-slim-buster as builder
RUN apt install librust-alsa-sys-dev
#RUN apt-get install -y libasound2-dev portaudio19-dev build-essential libpulse-dev libdbus-1-dev 
RUN apt-get install -y libsdl2-dev
RUN apt-get clean
WORKDIR /app
COPY ./ ./
WORKDIR /app/server
RUN cargo build --release

FROM rust:1.62-slim-buster as runner
COPY --from=builder /app/server/target/release/rust_scribble_server .

EXPOSE 3000

CMD ["./rust_scribble_server"]