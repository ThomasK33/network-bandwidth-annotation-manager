default:
	just --list

run: setup-k3d setup-tilt
stop: teardown-tilt teardown-k3d

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

# --- Examples ---

# Apply all examples
apply-examples: apply-annotation-example apply-strip-example apply-overwrite-example
# Delete all examples
delete-examples: delete-annotation-example delete-strip-example delete-overwrite-example

apply-annotation-example:
	kubectl apply -f deployments/annotator-mode.yaml

apply-strip-example:
	kubectl apply -f deployments/strip-mode.yaml

apply-overwrite-example:
	kubectl apply -f deployments/overwrite-mode.yaml

delete-annotation-example:
	kubectl delete -f deployments/annotator-mode.yaml

delete-strip-example:
	kubectl delete -f deployments/strip-mode.yaml

delete-overwrite-example:
	kubectl delete -f deployments/overwrite-mode.yaml
