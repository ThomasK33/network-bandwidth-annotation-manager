# Annotator Mode

By enabling bandwidth annotations on either a namespace level, by adding a `nbam-mode: "annotate"` label to the namespace, or the pod level, by adding the `nbam-mode: "annotate"` to the pod's annotations, NBAM will combine the network limits from each container and add the result to the corresponding annotations for CNIs to use.

=== "Example Namespace"

    ``` yaml linenums="1" hl_lines="6"
    apiVersion: v1
    kind: Namespace
    metadata:
    name: nbam-test
    labels:
        nbam-mode: "annotate"
    ```

=== "Before mutation"

    ```yaml linenums="1"
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

=== "After mutation"

    ```yaml linenums="1" hl_lines="8 9 12 13"
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
