# Twelve-Factor HTTP Server — Homework 1 (Rust)

A small HTTP server written in Rust and deployed to Kubernetes. It keeps its
visit counter in Redis, so the count is shared by all replicas, and it follows
the Twelve-Factor App methodology.

This is the Rust rewrite of the original Go `first-hw` implementation. The
observable interface is the one required by
[homework 1](../homework/hw1-twelve-factor.html):

| Request | Response |
|---|---|
| `GET /` | Increments the Redis counter and returns `<greeting> (visit <n>, served by <hostname>)`, or `503` when Redis is unreachable |
| `GET /healthz` | `ok`, without touching Redis |
| `GET /slow` | Waits 3 seconds, then answers `200` |

Running the same binary with the single argument `reset` clears the counter and
exits without starting the HTTP server.

## Layout

The homework brief is written for Go. The Rust project keeps the same behaviour
with these equivalent files:

| Homework brief | This repository |
|---|---|
| `main.go` | `src/main.rs` plus `src/config.rs`, `src/logging.rs`, `src/store.rs`, `src/web.rs` |
| `go.mod`, `go.sum` | `Cargo.toml`, `Cargo.lock` |
| `Dockerfile` | `Dockerfile` (multi-stage, distroless, non-root) |
| `k8s/redis.yaml` | `k8s/redis.yaml` |
| `k8s/deployment.yaml` | `k8s/deployment.yaml` |
| `k8s/service.yaml` | `k8s/service.yaml` |
| `README.md` | this file |

The simple lesson server used by homework 2 lives separately in `../hello/` and
is tagged `hello:0.1.1`. This homework application is tagged `hello:0.2.0`.

## Configuration

Everything comes from the environment:

| Variable | Default | Meaning |
|---|---|---|
| `PORT` | `8080` | port to listen on |
| `GREETING` | `hello from my container` | first part of the `/` answer |
| `REDIS_URL` | `redis://localhost:6379/0` | where the backing service is |

## Run it locally

Start Redis on the host:

```bash
docker run -d --name redis -p 6379:6379 redis:8-alpine
docker exec redis redis-cli ping
```

Then run the server and send a few requests:

```bash
cargo run
curl localhost:8080/
curl localhost:8080/
curl localhost:8080/healthz
curl localhost:8080/slow
```

Reset the shared counter with the administrative mode:

```bash
cargo run -- reset
```

Run the tests:

```bash
cargo test
```

## Build and deploy

```bash
docker build -t hello:0.2.0 .
kind load docker-image hello:0.2.0
kubectl apply -f k8s/
kubectl rollout status deployment/hello
```

Check the homework behaviour:

```bash
bash ../../scripts/check-12factor.sh hello:0.2.0
```

The path assumes this repository sits inside the `kubernetes-and-devops`
checkout as it does in this workspace. Adjust it if your checkout is elsewhere.

Run the one-off reset process in the cluster:

```bash
kubectl run reset --rm -it --restart=Never --image=hello:0.2.0 \
  --env=REDIS_URL=redis://redis:6379/0 -- reset
```

## Twelve factors

| # | Factor | Where this repository meets it |
|---:|---|---|
| I | Codebase | One Git repository contains the Rust server, tests, Dockerfile and Kubernetes manifests. |
| II | Dependencies | All crates are declared in `Cargo.toml` and pinned in `Cargo.lock`. The image is self-contained; nothing is installed on the host at run time. |
| III | Config | `PORT`, `GREETING` and `REDIS_URL` are read from the environment and set in `k8s/deployment.yaml`. |
| IV | Backing services | Redis is reached only through `REDIS_URL`. When it is unreachable, `GET /` answers `503`. |
| V | Build, release, run | `Cargo build` happens in the Dockerfile; the same image runs locally and in Kubernetes with different environment variables. |
| VI | Processes | The server is stateless. The only application state, the visit counter, lives in Redis and is incremented atomically with `INCR`. |
| VII | Port binding | The server binds TCP itself on `$PORT` and serves HTTP directly. |
| VIII | Concurrency | Kubernetes runs two replicas behind the `hello` Service. The shared Redis counter stays correct across them. |
| IX | Disposability | Startup is immediate. On `SIGTERM`/`SIGINT` the server stops accepting connections and waits for in-flight requests, including `/slow`, before exiting `0`. |
| X | Dev/prod parity | The same `hello:0.2.0` image and the same `redis:8-alpine` image are used locally and in kind. |
| XI | Logs | Every event is one JSON object per line on stdout, including one request event with method, path, status and duration. The service does not write to stderr. |
| XII | Admin processes | `reset` runs the same binary as a one-off process, clears the counter and exits without starting the HTTP server. |

## How the behaviour is implemented

- **Handlers** (`src/web.rs`): `/healthz` never calls Redis. `/` increments the
  counter and formats the required answer. `/slow` sleeps for three seconds.
- **Clock and shutdown** (`src/main.rs`): `axum::serve(...).with_graceful_shutdown`
  stops accepting new connections after `SIGTERM`/`SIGINT`, then waits for active
  requests. The process exits `0` when that is complete.
- **Redis** (`src/store.rs`): `INCR visits` is atomic; `SET visits 0` implements
  `reset`. Each operation has a two-second timeout so a dead backing service
  produces a fast `503` instead of a hanging request.
- **Logging** (`src/logging.rs`, `src/web.rs`): a small middleware writes exactly
  one compact JSON object per request to stdout.
- **Probes** (`k8s/deployment.yaml`): startup, readiness and liveness all check
  `/healthz`, so a Redis outage never causes a liveness restart loop.
