FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p updater-server

FROM debian:bookworm-slim
COPY --from=build /src/target/release/updater-server /usr/local/bin/updater-server
ENV DATA_DIR=/data BIND=0.0.0.0:8080
VOLUME /data
EXPOSE 8080
CMD ["updater-server"]
