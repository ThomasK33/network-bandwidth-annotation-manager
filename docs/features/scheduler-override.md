# Scheduler Override

By enabling the default scheduler override on either a namespace level or pod level, by adding `nbam-default-scheduler: "[SCHEDULER_NAME]"`, NBAM will override the default scheduler to the one defined in the label.

=== "Example Namespace"

    ```yaml linenums="1" hl_lines="6"
    apiVersion: v1
    kind: Namespace
    metadata:
      name: nbam-test
      labels:
        nbam-default-scheduler: my-scheduler
    ```

=== "Before mutation"

    ```yaml linenums="1"
    apiVersion: v1
    kind: Pod
    metadata:
      name: default-scheduler-overwrite
    spec:
      containers:
      - name: pod-with-second-annotation-container
        image: registry.k8s.io/pause:2.0
    ```

=== "After mutation"

    ```yaml linenums="1" hl_lines="6"
    apiVersion: v1
    kind: Pod
    metadata:
      name: default-scheduler-overwrite
    spec:
      schedulerName: my-scheduler
      containers:
      - name: pod-with-second-annotation-container
        image: registry.k8s.io/pause:2.0
    ```
