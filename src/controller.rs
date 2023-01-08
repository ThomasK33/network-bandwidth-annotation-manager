use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use color_eyre::Result;
use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::ListParams,
    core::ObjectMeta,
    runtime::{watcher, WatchStreamExt},
    Api, Client, Resource, ResourceExt,
};
use tokio::task::JoinHandle;

pub(crate) fn run(namespaces: Arc<Mutex<HashMap<String, ObjectMeta>>>) {
    let _handle: JoinHandle<()> = tokio::spawn({
        let namespaces_2 = namespaces.clone();

        async move {
            let _: Result<()> = async move {
                let client = Client::try_default().await?;

                watcher(Api::<Namespace>::all(client), ListParams::default())
                    .applied_objects()
                    .try_for_each(|namespace| {
                        let namespaces = namespaces.clone();

                        async move {
                            let name = namespace.name_any();
                            let meta = namespace.meta();

                            if let Ok(mut namespaces) = namespaces.lock() {
                                namespaces.insert(name, meta.clone());
                            }

                            Ok(())
                        }
                    })
                    .await?;

                Ok(())
            }
            .await;

            // TODO: Handle potential error from above closure

            run(namespaces_2)
        }
    });

    // TODO: Implement graceful shutdown
}

// TODO: Add e2e tests
