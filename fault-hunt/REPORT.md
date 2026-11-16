# Fault hunt report

Eight faults: six in `deployment.yaml`, two in `service.yaml`. Each section
records the symptom first, then the command and the exact evidence line that
named the cause, then the fix.

## Fault 1 — CPU request has no unit

- **Symptom:** Both Pods stay `Pending` and never receive a node.
- **Command:** `kubectl get pods` followed by `kubectl describe pod <pod>`.
- **Evidence:** `0/1 nodes are available: 1 Insufficient cpu.`
- **Cause:** `requests.cpu: "64"` is a valid Kubernetes quantity but means 64
  whole CPUs, so no node can satisfy the request.
- **Fix:** `fault-hunt/deployment.yaml`, `resources.requests.cpu` changed from
  `"64"` to `"64m"`.

## Fault 2 — Wrong image tag

- **Symptom:** After the Pods are schedulable, they stay in `ImagePullBackOff`.
- **Command:** `kubectl describe pod <pod>`.
- **Evidence:** `Failed to pull image "hello:0.1.10": ... not found` and
  `Back-off pulling image "hello:0.1.10"`.
- **Cause:** `hello:0.1.10` does not exist; the image built in lesson 07 is
  `hello:0.1.1`.
- **Fix:** `fault-hunt/deployment.yaml`, `spec.template.spec.containers[0].image`
  changed from `hello:0.1.10` to `hello:0.1.1`.

## Fault 3 — Memory limit too low

- **Symptom:** The new Pod starts and immediately enters `CrashLoopBackOff`.
- **Command:** `kubectl describe pod <pod>` and `kubectl get events`.
- **Evidence:** `container init was OOM-killed (memory limit too low?)` and
  `Last State: Terminated, Reason: OOMKilled`.
- **Cause:** `limits.memory: 2Mi` is below what the container needs during
  startup.
- **Fix:** `fault-hunt/deployment.yaml`, `resources.limits.memory` changed from
  `2Mi` to `128Mi`.

## Fault 4 — Invalid PORT value

- **Symptom:** The container still crash-loops after the memory fix.
- **Command:** `kubectl logs <pod> --previous`.
- **Evidence:** `listen tcp: lookup tcp/abc: unknown port`.
- **Cause:** The Deployment sets `PORT=abc`; the server cannot bind a port with
  that value and exits non-zero.
- **Fix:** `fault-hunt/deployment.yaml`, the `PORT` env value changed from
  `"abc"` to `"8080"`.

## Fault 5 — Readiness probe uses the wrong port

- **Symptom:** The Pod is `Running` but `READY 0/1`, so the Service has no
  ready endpoints.
- **Command:** `kubectl describe pod <pod>` and `kubectl get events`.
- **Evidence:** `Readiness probe failed: Get "http://10.244.0.7:8081/healthz":
  dial tcp 10.244.0.7:8081: connect: connection refused`.
- **Cause:** The readiness probe targets port `8081`, while the container
  listens on `8080`.
- **Fix:** `fault-hunt/deployment.yaml`,
  `readinessProbe.httpGet.port` changed from `8081` to `8080`.

## Fault 6 — Liveness probe uses the wrong port

- **Symptom:** The Pod becomes ready but is restarted repeatedly.
- **Command:** `kubectl describe pod <pod>` and `kubectl get events`.
- **Evidence:** `Liveness probe failed: Get "http://10.244.0.9:8008/healthz":
  dial tcp 10.244.0.9:8008: connect: connection refused`, followed by
  `Container hello failed liveness probe, will be restarted`.
- **Cause:** The liveness probe targets port `8008`, while the container listens
  on `8080`.
- **Fix:** `fault-hunt/deployment.yaml`,
  `livenessProbe.httpGet.port` changed from `8008` to `8080`.

## Fault 7 — Service selector matches no Pods

- **Symptom:** The Deployment is healthy but `GET /` through the Service fails
  and the Service has no endpoints.
- **Command:** `kubectl describe service hello` and
  `kubectl get endpointslices`.
- **Evidence:** `Selector: app=hello,tier=web` with an empty `Endpoints:` field;
  the endpoint slice lists no ready addresses.
- **Cause:** The Service selects `app=hello` **and** `tier=web`, but the Pod
  template only carries `app=hello`, so no Pod is selected.
- **Fix:** `fault-hunt/service.yaml`, removed `tier: web` from `spec.selector`.

## Fault 8 — Service targetPort points at the wrong container port

- **Symptom:** Endpoints appear after the selector fix, but connecting through
  the Service is still refused.
- **Command:** `kubectl describe service hello` and `kubectl get endpointslices`.
- **Evidence:** `TargetPort: 80/TCP` while the Pod's only container port is
  `8080`; `kubectl port-forward svc/hello 8080:80` then
  `curl localhost:8080/` returns connection refused.
- **Cause:** The Service forwards to container port `80`, where nothing listens.
- **Fix:** `fault-hunt/service.yaml`, `targetPort` changed from `80` to `8080`.
