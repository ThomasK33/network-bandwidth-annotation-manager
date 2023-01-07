# Strip Mode

By enabling the extended resources stripper feature on a namespace level, by adding `nbam-mode: "strip"` label to the namespace, or on a pod level, by adding the `nbam-mode: "strip"` to the pod's annotations, NBAM will perform the same operations as in the annotator flag feature while additionally stripping the extended sources from the object.

=== "Example Namespace"

    ```yaml linenums="1" hl_lines="6"
    apiVersion: v1
    kind: Namespace
    metadata:
      name: nbam-test
      labels:
        nbam-mode: "strip"
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
            # Limits the ingress bandwidth to 2Mbit/s
            networking.k8s.io/ingress-bandwidth: 2M
            # Limits the egress bandwidth to 2Mbit/s
            networking.k8s.io/egress-bandwidth: 2M
    ```

=== "After mutation"

    ```yaml linenums="1" hl_lines="7 8 9 10"
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
