# Introduction

The network bandwidth annotation manager is a dynamic admission controller for Kubernetes setting a pod's network bandwidth annotations using its limits, stripping [extended resources], and optionally changing a pod's scheduler.

The primary motivation behind creating nbam is the ability to use extended resource FQDNs in pod resource requests, as many helm charts or other packaged Kubernetes deployments do not allow setting custom pod annotations, as required by the CNI spec.
Yet, one can usually set CPU and memory limits in helm charts or Kubernetes primitives. Thus nbam takes care of rewriting those to the corresponding pod annotations in multiple modes.

## Features

By adding specific labels to namespaces, NBAM mutates pod definitions accordingly to the features enabled.

These mutations occur before the object's persistence by the apiserver.
Thus, the kube-scheduler and CNI can use the object without further changes.

Currently available features are:

- [[annotator-mode|Annotator Mode]]
- [[overwrite-mode|Overwrite Mode]]
- [[strip-mode|Strip Mode]]
- [[scheduler-override|Scheduler Override]]

[extended resources]: https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/#extended-resources
