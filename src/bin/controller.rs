use std::fmt;
use std::hash::{self, Hasher as _};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use futures::StreamExt as _;
use json_patch::{PatchOperation, ReplaceOperation};
use kube::api::{ApiResource, DynamicObject, PartialObjectMeta, Patch, PatchParams};
use kube::runtime::watcher::{self, InitialListStrategy};
use kube::runtime::{controller, Controller};
use kube::{discovery, ResourceExt};
use kube_subcontroller::config::subcontroller;
use kube_subcontroller::subscriber;
use shardlabel::api::{ShardingRule, SHARD_LABEL};

#[derive(serde::Deserialize)]
struct Config {
    enable_stream_list: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ::config::Config::builder()
        .add_source(config::Environment::with_prefix("KUBE_SHARD_LABEL_"))
        .build()
        .context("construct config from env")?
        .try_deserialize::<Config>()
        .context("deserialize env to config")?;

    let client = kube::Client::try_default().await.context("init kube client")?;

    let rule_client = kube::Api::<ShardingRule>::all_with(client.clone(), &());
    kube_subcontroller::run(
        kube_subcontroller::on(move || {
            subscriber::objects(rule_client.clone(), watcher::Config::default())
        })
        .with(move || {
            Box::new(subcontroller({
                let client = client.clone();
                move |_cancel, _key, rule: ShardingRule| {
                    let client = client.clone();
                    let rule = Arc::new(rule);

                    async move {
                        let group = discovery::oneshot::group(&client, &rule.spec.group)
                            .await
                            .context("lookup resource referenced by rule")?;
                        let (resource, _capab) = group
                            .recommended_kind(&rule.spec.kind)
                            .context("no resource in group with kind")?;
                        let resource = Arc::new(resource);

                        let api = kube::Api::<PartialObjectMeta<DynamicObject>>::all_with(
                            client.clone(),
                            &resource,
                        );
                        Controller::new_with(
                            api,
                            watcher::Config {
                                initial_list_strategy: if config.enable_stream_list {
                                    InitialListStrategy::StreamingList
                                } else {
                                    InitialListStrategy::ListWatch
                                },
                                ..<_>::default()
                            },
                            ApiResource::clone(&resource),
                        )
                        .run(
                            |object, _ctx| {
                                let rule = rule.clone();
                                let client = client.clone();
                                let resource = resource.clone();
                                async move { reconcile(&rule, &object, &client, &resource).await }
                            },
                            |_, _, _| controller::Action::requeue(Duration::from_secs(1)),
                            Arc::new(()),
                        )
                        .for_each(|result| async move {
                            dbg!(result);
                        })
                        .await;

                        anyhow::Ok(())
                    }
                }
            }))
        }),
    )
    .await
    .context("run controller")
}

async fn reconcile(
    rule: &ShardingRule,
    object: &PartialObjectMeta<DynamicObject>,
    client: &kube::Client,
    api_resource: &ApiResource,
) -> Result<controller::Action, ReconcileError> {
    let num_shards = rule.spec.sharding_count;

    let shard_index = {
        let mut hasher = fnv::FnvHasher::default();
        hash::Hash::hash(
            &(object.metadata.namespace.as_deref(), object.metadata.name.as_deref()),
            &mut hasher,
        );
        let cksum = hasher.finish();
        (cksum % num_shards as u64) as i32
    };

    let label_value = format!("{shard_index}/{num_shards}");

    if object.labels().get(SHARD_LABEL) != Some(&label_value) {
        let api: kube::Api<PartialObjectMeta<DynamicObject>> = match &object.metadata.namespace {
            Some(ns) => kube::Api::namespaced_with(client.clone(), ns, api_resource),
            None => kube::Api::all_with(client.clone(), api_resource),
        };

        let patch: Patch<()> =
            Patch::Json(json_patch::Patch(vec![PatchOperation::Replace(ReplaceOperation {
                path:  format!("/metadata/labels/{}", Rfc6901Encode(SHARD_LABEL)),
                value: serde_json::Value::String(label_value),
            })]));

        api.patch_metadata(
            object.metadata.name.as_deref().unwrap_or_default(),
            &PatchParams::default(),
            &patch,
        )
        .await
        .map_err(ReconcileError::PatchRequest)?;
    }

    Ok(controller::Action::await_change())
}

#[derive(Debug, thiserror::Error)]
enum ReconcileError {
    #[error("applying label patch: {0}")]
    PatchRequest(kube::Error),
}

struct Rfc6901Encode<'a>(&'a str);

impl<'a> fmt::Display for Rfc6901Encode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut str = self.0;
        while !str.is_empty() {
            if str.strip_prefix('~').is_some() {
                f.write_str("~0")?;
            } else if str.strip_prefix('/').is_some() {
                f.write_str("~1")?;
            } else {
                let until = str.find(|ch| ch == '~' || ch == '/').unwrap_or(str.len());
                let (front, back) = str.split_at(until);
                str = back;
                f.write_str(front)?;
            }
        }

        Ok(())
    }
}
