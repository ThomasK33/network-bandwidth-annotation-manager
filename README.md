# Network Bandwidth Annotation Manager

The network bandwidth annotation manager is a dynamic admission controller for Kubernetes setting a pod's network bandwidth annotations using its limits, stripping [extended resources], and optionally changing a pod's scheduler.

The primary motivation behind creating nbam is the ability to use extended resource FQDNs in pod resource requests, as many helm charts or other packaged Kubernetes deployments do not allow setting custom pod annotations, as required by the CNI spec.
Yet, one can usually set CPU and memory limits in helm charts or Kubernetes primitives. Thus nbam takes care of rewriting those to the corresponding pod annotations in multiple modes.

## Features

By adding specific labels to namespaces, NBAM mutates pod definitions accordingly to the features enabled.

These mutations occur before the object's persistence by the apiserver.
Thus, the kube-scheduler and CNI can use the object without further changes.

One can find a feature overview in the [project documentation's feature section].

## Build

### Binary

One can build a release binary using the following:

```bash
cargo build --release
```

### OCI

One can create the OCI image using the following:

```bash
docker build -t nbam:latest "."
```

## Usage

### CLI usage

```bash
network-bandwidth-annotation-manager --listen 0.0.0.0:8443 --tls-cert ./cert.pem --tls-key ./key.pem
```

### Kubernetes Deployment

The following example of a Kubernetes deployment assumes one installed cert-manager and its webhook correctly.

One can find an example deployment at [`deployments/manager.yaml`](deployments/manager.yaml).

## Example

A prerequisite for setting up a local development environment is installing [k3d], [tilt], and [just] locally.

By running the following, one will create a local environment consisting of a [k3d] managed registry, [k3d] multi-node cluster, and [tilt]:

```bash
just run
```

To add networking-related node capacities and allocatable amounts, open a new shell instance, leaving the previous one open, and run the following:

```bash
just annotate-nodes
```

One can then inspect all resources and allocations using, e.g., [kubectl-view-allocations].

```bash
kubectl view-allocations
```

To apply all examples listed above, one should use the following:

```bash
just apply-examples
```

To stop the local development environment, one should run the following:

```bash
just stop
```

## Contributing

### Documentation

To generate the license file, followed by `mkdocs serve`, one can run the following:

```bash
just docs
```

[extended resources]: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/#extended-resources
[k3d]: https://k3d.io/v5.4.6/
[tilt]: https://tilt.dev/
[just]: https://github.com/casey/just
[project documentation's feature section]: https://thomask33.github.io/network-bandwidth-annotation-manager/features/annotator-mode/
[kubectl-view-allocations]: (https://github.com/davidB/kubectl-view-allocations)
