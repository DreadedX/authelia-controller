use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Secret;
use kube::api::{ObjectMeta, Patch, PatchParams};
use kube::{Api, CustomResource, ResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use crate::context::Context;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, JsonSchema, Hash)]
#[serde(rename_all = "snake_case")]
enum AccessPolicy {
    Deny,
    Bypass,
    OneFactor,
    TwoFactor,
}

#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, JsonSchema, Hash)]
#[kube(
    kind = "AccessControlRule",
    group = "authelia.huizinga.dev",
    version = "v1"
)]
#[kube(
    shortname = "acl",
    doc = "Custom resource for managing authelia access rules"
)]
#[serde(rename_all = "camelCase")]
pub struct AccessControlRuleSpec {
    domain: String,
    policy: AccessPolicy,
    subject: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
struct AccessControl {
    rules: Vec<AccessControlRuleSpec>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash)]
struct TopLevel {
    access_control: AccessControl,
}

impl AccessControlRule {
    pub async fn update_acl(
        mut rules: Vec<Arc<AccessControlRule>>,
        ctx: Arc<Context>,
    ) -> Result<(), kube::Error> {
        debug!("Updating acl");
        rules.sort_by_cached_key(|rule| rule.name_any());

        let rules = rules
            .iter()
            .inspect(|rule| trace!(name = rule.name_any(), "Rule found"))
            .map(|rule| rule.spec.clone())
            .collect();

        let top = TopLevel {
            access_control: AccessControl { rules },
        };

        let contents = BTreeMap::from([(
            "configuration.acl.yaml".into(),
            serde_yaml::to_string(&top).expect("serializer should not fail"),
        )]);

        let secret = Secret {
            metadata: ObjectMeta {
                name: Some(ctx.secret_name.clone()),
                ..Default::default()
            },
            string_data: Some(contents),
            ..Default::default()
        };

        debug!(
            name = ctx.secret_name,
            namespace = ctx.namespace,
            "Applying secret"
        );
        let secrets = Api::<Secret>::namespaced(ctx.client.clone(), &ctx.namespace);
        secrets
            .patch(
                &ctx.secret_name,
                &PatchParams::apply(&ctx.controller_name),
                &Patch::Apply(&secret),
            )
            .await?;

        let mut hasher = DefaultHasher::new();
        top.hash(&mut hasher);
        let hash = hasher.finish();

        let patch = serde_json::json!({
            "spec": {
                "template": {
                    "metadata": {
                        "annotations": {
                            "authelia.huizinga.dev/aclHash": hash.to_string()
                        }
                    }
                }
            }
        });

        debug!(
            name = ctx.deployment_name,
            namespace = ctx.namespace,
            hash,
            "Updating deployment hash"
        );
        let deployments = Api::<Deployment>::namespaced(ctx.client.clone(), &ctx.namespace);
        deployments
            .patch(
                &ctx.deployment_name,
                &PatchParams::default(),
                &Patch::Strategic(&patch),
            )
            .await?;

        Ok(())
    }
}
