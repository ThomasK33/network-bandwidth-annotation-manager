# Network Bandwidth Annotation Manager

The network bandwidth annotation manager is a dynamic admission controller for Kubernetes setting a pod's network bandwidth annotations using its limits, stripping [extended resources], and optionally changing a pod's scheduler.

The primary motivation behind creating nbam is the ability to use extended resource FQDNs in pod resource requests, as many helm charts or other packaged Kubernetes deployments do not allow setting custom pod annotations, as required by the CNI spec.
Yet, one can usually set CPU and memory limits in helm charts or Kubernetes primitives. Thus nbam takes care of rewriting those to the corresponding pod annotations in multiple modes.

## Features

By adding specific labels to namespaces, NBAM mutates pod definitions accordingly to the features enabled.

These mutations occur before the object's persistence by the apiserver.
Thus, the kube-scheduler and CNI can use the object without further changes.

### Annotator Mode

By enabling bandwidth annotations on either a namespace level, by adding a `nbam-mode: "annotate"` label to the namespace, or the pod level, by adding the `nbam-mode: "annotate"` to the pod's annotations, NBAM will combine the network limits from each container and add the result to the corresponding annotations for CNIs to use.

<details>
<summary>Example Namespace</summary>

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: nbam-test
  labels:
    nbam-mode: "annotate"
```

</details>

<details>
 <summary>Before mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
      limits:
        cpu: 4
        # Limits the ingress bandwidth to 1Mbit/s
        networking.k8s.io/ingress-bandwidth: 1M
        # Limits the egress bandwidth to 1Mbit/s
        networking.k8s.io/egress-bandwidth: 1M
```

</details>

<details>
<summary>After mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
  annotations:
    # These annotations are used by CNIs for traffic shaping
    kubernetes.io/ingress-bandwidth: 1M
    kubernetes.io/egress-bandwidth: 1M
    # These additional annotations are set by the mutating webhook for
    # use with custom schedulers
    kubernetes.io/ingress-request: 1M
    kubernetes.io/egress-request: 1M
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
      limits:
        cpu: 4
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
```

</details>

### Strip Mode

By enabling the extended resources stripper feature on a namespace level, by adding `nbam-mode: "strip"` label to the namespace, or on a pod level, by adding the `nbam-mode: "strip"` to the pod's annotations, NBAM will perform the same operations as in the annotator flag feature while additionally stripping the extended sources from the object.

<details>
<summary>Example Namespace</summary>

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: nbam-test
  labels:
    nbam-mode: "strip"
```

</details>

<details>
 <summary>Before mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
      limits:
        cpu: 4
        # Limits the ingress bandwidth to 2Mbit/s
        networking.k8s.io/ingress-bandwidth: 2M
        # Limits the egress bandwidth to 2Mbit/s
        networking.k8s.io/egress-bandwidth: 2M
```

</details>

<details>
<summary>After mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
  annotations:
    kubernetes.io/ingress-bandwidth: 2M
    kubernetes.io/egress-bandwidth: 2M
    kubernetes.io/ingress-request: 1M
    kubernetes.io/egress-request: 1M
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
      limits:
        cpu: 4
```

</details>

### Overwrite Mode

By enabling the extended resources overwrite feature on a namespace level, by adding `nbam-mode: "overwrite"` label to the namespace, or on a pod level, by adding the `nbam-mode: "overwrite"` to the pod's annotations, NBAM will perform the same operations as in the annotator flag feature while overriding each pod's networking limits with its networking requests.

This mode is useful for scheduling with extended resources yet still being able to overcommit and set higher limits on the CNI.

<details>
<summary>Example Namespace</summary>

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: nbam-test
  labels:
    nbam-mode: "overwrite"
```

</details>

<details>
 <summary>Before mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
      limits:
        cpu: 4
        # Limits the ingress bandwidth to 2Mbit/s
        networking.k8s.io/ingress-bandwidth: 2M
        # Limits the egress bandwidth to 2Mbit/s
        networking.k8s.io/egress-bandwidth: 2M
```

</details>

<details>
<summary>After mutation</summary>

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-pod
  namespace: nbam-test
  annotations:
    kubernetes.io/ingress-bandwidth: 4M
    kubernetes.io/egress-bandwidth: 4M
    kubernetes.io/ingress-request: 1M
    kubernetes.io/egress-request: 1M
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
      limits:
        cpu: 4
        networking.k8s.io/ingress-bandwidth: 1M
        networking.k8s.io/egress-bandwidth: 1M
```

</details>

### (WIP) Scheduler Override

By enabling the default scheduler override on either a namespace level or pod level, by adding `nbam-default-scheduler: "[SCHEDULER_NAME]"` , NBAM will override the default scheduler to the one defined in the label.

TODO: Add examples

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

One can find an example deployment at `./deployment.yaml`.

[extended resources]: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/#extended-resources
