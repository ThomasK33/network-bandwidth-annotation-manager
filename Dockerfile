FROM rust:1.66.1 as build-env
WORKDIR /app
COPY . /app
RUN cargo install cargo-auditable cargo-audit
RUN cargo auditable build --release

FROM gcr.io/distroless/cc
COPY --from=build-env /app/target/release/network-bandwidth-annotation-manager /
CMD ["./network-bandwidth-annotation-manager"]
