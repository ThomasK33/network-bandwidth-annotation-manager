# --- Extensions ---
# cert-managager
load('ext://cert_manager', 'deploy_cert_manager')
deploy_cert_manager()

# --- network-bandwidth-annotation-manager ---
docker_build(
	"thomask33/nbam",
	".",
	only=["Cargo.toml", "Cargo.lock", "Dockerfile", "./src/"]
)

k8s_yaml("deployments/manager.yaml")
k8s_resource(workload="network-bandwidth-annotation-manager", port_forwards=8443, labels=["operator"])
