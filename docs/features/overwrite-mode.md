# Overwrite Mode

By enabling the extended resources overwrite feature on a namespace level, by adding `nbam-mode: "overwrite"` label to the namespace, or on a pod level, by adding the `nbam-mode: "overwrite"` to the pod's annotations, NBAM will perform the same operations as in the annotator flag feature while overriding each pod's networking limits with its networking requests.

This mode is useful for scheduling with extended resources yet still being able to overcommit and set higher limits on the CNI.

=== "Example Namespace"

    ```yaml linenums="1" hl_lines="6"
    apiVersion: v1
    kind: Namespace
    metadata:
      name: nbam-test
    labels:
      nbam-mode: "overwrite"
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

    ```yaml linenums="1" hl_lines="7 8 9 10 22 23"
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
            networking.k8s.io/ingress-bandwidth: 1M
            networking.k8s.io/egress-bandwidth: 1M
          limits:
            cpu: 4
            networking.k8s.io/ingress-bandwidth: 1M
            networking.k8s.io/egress-bandwidth: 1M
    ```
