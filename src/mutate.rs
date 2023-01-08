use axum::{http::StatusCode, response::IntoResponse, Json};
use color_eyre::{eyre::ContextCompat, Result};
use json_patch::{AddOperation, CopyOperation, PatchOperation, RemoveOperation};
use kube::{
    core::{
        admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
        DynamicObject,
    },
    Resource, ResourceExt,
};
use tracing::{error, info, warn};

use crate::{
    utils::{escape_json_pointer, quantity},
    NamespaceCache,
};

pub(crate) enum Mode {
    Bandwidth(BandwidthProps),
    Scheduler(NamespaceCache),
}

pub(crate) struct BandwidthProps {
    pub(crate) egress_bandwidth_resource_key: String,
    pub(crate) ingress_bandwidth_resource_key: String,
    pub(crate) mode: BandwidthMode,
}

pub(crate) enum BandwidthMode {
    Annotate,
    Strip,
    Overwrite,
}

// A general /mutate handler, handling errors from the underlying business logic
pub(crate) async fn handler(
    Json(body): Json<AdmissionReview<DynamicObject>>,
    mode: Mode,
) -> impl IntoResponse {
    // Parse incoming webhook AdmissionRequest first
    let req: AdmissionRequest<_> = match body.try_into() {
        Ok(req) => req,
        Err(err) => {
            error!("invalid request: {}", err.to_string());
            return (
                StatusCode::BAD_REQUEST,
                Json(AdmissionResponse::invalid(err.to_string()).into_review()),
            );
        }
    };

    // Then construct a AdmissionResponse
    let mut res = AdmissionResponse::from(&req);

    // req.Object always exists for us, but could be None if extending to DELETE events
    if let Some(obj) = req.object {
        let name = obj.name_any(); // apiserver may not have generated a name yet

        res = match match mode {
            Mode::Bandwidth(props) => mutate_bandwidth(
                res.clone(),
                &obj,
                &props.egress_bandwidth_resource_key,
                &props.ingress_bandwidth_resource_key,
                props.mode,
            ),
            Mode::Scheduler(cache) => mutate_scheduler(res.clone(), &obj, cache),
        } {
            Ok(res) => {
                // TODO: Remove those verbose logs
                info!("accepted: {:?} on pod {}", req.operation, name);

                res
            }
            Err(err) => {
                warn!("denied: {:?} on {} ({})", req.operation, name, err);
                res.deny(err.to_string())
            }
        };
    };

    // Wrap the AdmissionResponse wrapped in an AdmissionReview
    (StatusCode::OK, Json(res.into_review()))
}

// The main handler and core business logic, failures here implies rejected applies
fn mutate_bandwidth(
    res: AdmissionResponse,
    obj: &DynamicObject,
    egress_bandwidth_resource_key: &str,
    ingress_bandwidth_resource_key: &str,
    mode: BandwidthMode,
) -> Result<AdmissionResponse> {
    let mut patches = Vec::new();

    // If the resource doesn't contain "admission", we add it to the resource.
    if !obj.annotations().contains_key("nba-admission") {
        // Ensure annotations exist before adding a key to it
        if obj.meta().annotations.is_none() {
            patches.push(PatchOperation::Add(AddOperation {
                path: "/metadata/annotations".into(),
                value: serde_json::json!({}),
            }));
        }

        // Add our annotation
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/nba-admission".into(),
            value: serde_json::Value::String("true".into()),
        }));
    };

    let mut egress_request: Option<f64> = None;
    let mut ingress_request: Option<f64> = None;
    let mut egress_limit: Option<f64> = None;
    let mut ingress_limit: Option<f64> = None;

    if let Some(containers) = obj.data.get("spec").and_then(|spec| {
        spec.get("containers")
            .and_then(|containers| containers.as_array())
    }) {
        for (index, container) in containers.iter().enumerate() {
            let Some(resources) = container.get("resources") else { continue };

            // -- Get egress and ingress requests --
            let requests = resources.get("requests").map(|requests| {
                (
                    requests
                        .get(egress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity::parse),
                    requests
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity::parse),
                )
            });

            if let Some((egress, ingress)) = &requests {
                if let Some(Ok(egress)) = egress {
                    if egress_request.is_none() {
                        egress_request = Some(0.0);
                    }

                    if let Some(egress_request) = egress_request.as_mut() {
                        *egress_request += egress;
                    }
                }

                if let Some(Ok(ingress)) = ingress {
                    if ingress_request.is_none() {
                        ingress_request = Some(0.0);
                    }

                    if let Some(ingress_request) = ingress_request.as_mut() {
                        *ingress_request += ingress;
                    }
                }
            }

            // -- Get egress and ingress limits --
            let limits = resources.get("limits").map(|limits| {
                (
                    limits
                        .get(egress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity::parse),
                    limits
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity::parse),
                )
            });

            if let Some((egress, ingress)) = &limits {
                if let Some(Ok(egress)) = egress {
                    if egress_limit.is_none() {
                        egress_limit = Some(0.0);
                    }

                    if let Some(egress_limit) = egress_limit.as_mut() {
                        *egress_limit += egress;
                    }
                }

                if let Some(Ok(ingress)) = ingress {
                    if ingress_limit.is_none() {
                        ingress_limit = Some(0.0);
                    }

                    if let Some(ingress_limit) = ingress_limit.as_mut() {
                        *ingress_limit += ingress;
                    }
                }
            }

            // -- Mutation modes --
            match mode {
                BandwidthMode::Annotate => {
                    // In annotate mode, no further operations have to be performed on the Kubernetes object
                    // thus it's a noop
                }
                // Strip custom bandwidth resource requests and limits if strip = true
                BandwidthMode::Strip => {
                    if let Some((egress, ingress)) = requests {
                        if let Some(Ok(_)) = egress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        if let Some(Ok(_)) = ingress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }

                    if let Some((egress, ingress)) = limits {
                        if let Some(Ok(_)) = egress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        if let Some(Ok(_)) = ingress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }
                }
                BandwidthMode::Overwrite => {
                    // Check if current container has both, requests and limits set
                    if let (Some(limits), Some(requests)) = (limits, requests) {
                        // Check if current container has egress bandwidth defined in both requests and limits set
                        if let (Some(Ok(_)), Some(Ok(_))) = (limits.0, requests.0) {
                            patches.push(PatchOperation::Copy(CopyOperation {
                                from: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        // Check if current container has ingress bandwidth defined in both requests and limits set
                        if let (Some(Ok(_)), Some(Ok(_))) = (limits.1, requests.1) {
                            patches.push(PatchOperation::Copy(CopyOperation {
                                from: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }
                }
            };
        }
    }

    // Add request annotations for use-cases with dedicated schedulers
    if let Some(egress_request) = egress_request {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1egress-request".into(),
            value: serde_json::Value::String(egress_request.to_string()),
        }));
    }
    if let Some(ingress_request) = ingress_request {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1ingress-request".into(),
            value: serde_json::Value::String(ingress_request.to_string()),
        }));
    }

    // Add annotations to pod if limits exist
    if let Some(egress_limit) = egress_limit {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1egress-bandwidth".into(),
            value: serde_json::Value::String(egress_limit.to_string()),
        }));
    }
    if let Some(ingress_limit) = ingress_limit {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1ingress-bandwidth".into(),
            value: serde_json::Value::String(ingress_limit.to_string()),
        }));
    }

    Ok(if !patches.is_empty() {
        res.with_patch(json_patch::Patch(patches))?
    } else {
        res
    })
}

fn mutate_scheduler(
    res: AdmissionResponse,
    obj: &DynamicObject,
    namespaces: NamespaceCache,
) -> Result<AdmissionResponse> {
    let mut patches = Vec::new();

    let key = "nbam-default-scheduler";

    // Check if the pod has a scheduler label
    let scheduler_name = if let Some(default_scheduler) = obj.labels().get(key) {
        default_scheduler.to_owned()
    } else {
        let obj_ns = obj.namespace().context(format!(
            "Could not determine namespace for object: {}",
            obj.name_any()
        ))?;

        // Otherwise try obtaining the scheduler name from the namespace cache
        let namespaces = namespaces
            .lock()
            .map_err(|err| color_eyre::eyre::eyre!("Could not acquire namespace cache: {err}"))?;

        let namespace = namespaces.get(&obj_ns).context(format!(
            "Failed to get namespace \"{obj_ns}\" from namespace cache"
        ))?;

        namespace
            .labels
            .as_ref()
            .context(format!(
                "Namespace \"{obj_ns}\" does not have any labels set"
            ))?
            .get(key)
            .context(format!("Label \"{key}\" missing in namespace \"{obj_ns}\""))?
            .to_owned()
    };

    patches.push(PatchOperation::Add(AddOperation {
        path: "/spec/schedulerName".into(),
        value: serde_json::Value::String(scheduler_name),
    }));

    Ok(if !patches.is_empty() {
        res.with_patch(json_patch::Patch(patches))?
    } else {
        res
    })
}

// TODO: Add tests
