_default:
	just --list

# Run local development environment
run: setup-k3d setup-tilt
# Stop local development environment
stop: teardown-tilt teardown-k3d

# Run mkdocs locally
docs:
	cargo about generate about.hbs > docs/license.md
	mkdocs serve

# --- tilt ---

# Start tilt
setup-tilt:
	tilt up

# Shutdown tilt
teardown-tilt:
	tilt down

# --- k3d ---

# Setup local k3d cluster and registry
setup-k3d:
	k3d registry create default-registry.localhost --port 9090
	k3d cluster create default --servers 3 --registry-use k3d-default-registry.localhost:9090

# Delete local k3d cluster and registry
teardown-k3d:
	k3d cluster delete default
	k3d registry delete default-registry.localhost

# - Node Bandwidth Annotations -

# Add networking-related extended resources to Kubernetes nodes
annotate-nodes:
	#!/usr/bin/env bash
	kubectl proxy &
	K_PID=$!

	while ! nc -z localhost 8001; do
		sleep 0.1 # wait for 1/10 of the second before check again
	done

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1ingress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-0/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1egress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-0/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1ingress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-1/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1egress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-1/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1ingress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-2/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/capacity/networking.k8s.io~1egress-bandwidth", "value": "1.25e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-2/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1ingress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-0/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1egress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-0/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1ingress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-1/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1egress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-1/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1ingress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-2/status

	curl --header "Content-Type: application/json-patch+json" \
		--request PATCH \
		--data '[{"op": "add", "path": "/status/allocatable/networking.k8s.io~1egress-bandwidth", "value": "1e+9"}]' \
		http://localhost:8001/api/v1/nodes/k3d-default-server-2/status

	kill -9 $K_PID

# --- Examples ---

# Apply all examples
apply-examples:
	kubectl apply -f examples/annotator-mode.yaml
	kubectl apply -f examples/strip-mode.yaml
	kubectl apply -f examples/overwrite-mode.yaml
# Delete all examples
delete-examples:
	kubectl delete -f examples/annotator-mode.yaml
	kubectl delete -f examples/strip-mode.yaml
	kubectl delete -f examples/overwrite-mode.yaml
