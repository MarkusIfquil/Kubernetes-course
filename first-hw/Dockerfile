# syntax=docker/dockerfile:1

# Stage 1: build with the Rust toolchain.
FROM rust:1.100-bookworm AS build

WORKDIR /src

# Dependencies change rarely, so cache the dependency build before the source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Application code changes often, so it is copied last.
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Stage 2: distroless runtime, non-root by default.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=build /src/target/release/hello /server

EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/server"]
