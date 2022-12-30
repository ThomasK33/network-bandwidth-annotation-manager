# Network Bandwidth Annotation Manager

The network bandwidth annotation manager is a dynamic admission controller for Kubernetes setting a pod's network bandwidth annotations using its limits, stripping [extended resources], and optionally changing a pod's scheduler.

## Features

By adding specific labels to namespaces, NBAM mutates pod definitions accordingly to the features enabled.

These mutations occur before the object's persistence by the apiserver.
Thus, the kube-scheduler and CNI can use the object without further changes.

### Annotator flag

By enabling bandwidth annotations on either a namespace level, by adding a `nbam-enabled: "true"` label to the namespace, or the pod level, by adding the `nbam-enabled: "true"` to the pod's annotations, NBAM will combine the network limits from each container and add the result to the corresponding annotations for CNIs to use.

<details>
<summary>Example Namespace</summary>

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: nbam-test
  labels:
    nbam-enabled: "true"
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
        # if the pod contains ingress and egress bandwidth annotations
        # the requests will be automatically set to the annotations values
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
    # This is necessary because of the way CNI traffic shaping support
    # currently implements its limits
    kubernetes.io/ingress-bandwidth: 1M
    kubernetes.io/egress-bandwidth: 1M
spec:
  containers:
  - name: my-container
    image: nginx:1.23
    resources:
      requests:
        cpu: 2
        # if the pod contains ingress and egress bandwidth annotations
        # the requests will be automatically set to the annotations values
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

### (WIP) Extended Resources Stripper

By enabling the extended resources stripper feature on a namespace level, by adding `nbam-strip: "true"` label to the namespace, or on a pod level, by adding the `nbam-strip: "true"` to the pod's annotations, NBAM will perform the same operations as in the annotator flag feature while additionally stripping the extended sources from the object.

<details>
<summary>Example Namespace</summary>

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: nbam-test
  labels:
    nbam-strip: "true"
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
        # if the pod contains ingress and egress bandwidth annotations
        # the requests will be automatically set to the annotations values
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
    # This is necessary because of the way CNI traffic shaping support
    # currently implements its limits
    kubernetes.io/ingress-bandwidth: 1M
    kubernetes.io/egress-bandwidth: 1M
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
