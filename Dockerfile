FROM rust:1.66.0 as build-env
WORKDIR /app
COPY . /app
RUN cargo build --release

FROM gcr.io/distroless/cc
COPY --from=build-env /app/target/release/network-bandwidth-annotation-manager /
CMD ["./network-bandwidth-annotation-manager"]
