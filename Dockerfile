# syntax=docker/dockerfile:1

# Stage 1: build with the Rust toolchain.
FROM rust:1.100-bookworm AS build

WORKDIR /src

# Cache the dependency build before copying the source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && printf 'fn main() {}\n' > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

# An empty directory for the volume (distroless has no mkdir).
RUN mkdir /data

# Stage 2: distroless runtime, non-root by default.
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=build /src/target/release/hello /server
# The volume directory must belong to the user that writes to it.
COPY --from=build --chown=65532:65532 /data /data
VOLUME /data

EXPOSE 8080
USER 65532:65532
ENTRYPOINT ["/server"]
