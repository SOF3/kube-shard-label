use k8s_openapi::apimachinery::pkg::apis::meta;

pub const SHARD_LABEL: &str = "shardlabel.sof3.github.io/object-shard";

#[derive(
    schemars::JsonSchema, kube::CustomResource, serde::Serialize, serde::Deserialize, Debug, Clone,
)]
#[kube(
    group = "shardlabel.sof3.github.io",
    version = "v1alpha1",
    kind = "ShardingRule",
    plural = "shardingrules",
    status = "ShardingRuleStatus",
    printcolumn = r#"{
        "GROUP": ".spec.group",
        "KIND": ".spec.kind",
        "LOG #SHARDS": ".spec.logShardingCount",
        "OBJECTS": ".status.convertedObjects"
    }"#
)]
#[serde(rename_all = "camelCase")]
pub struct ShardingRuleSpec {
    /// The group of the resource type to match this sharding rule against.
    pub group:          String,
    /// The kind of the resource type to match this sharding rule against.
    pub kind:           String,
    /// Number of shards to partition this resource type into.
    #[schemars(range(min = 1))]
    pub sharding_count: i32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShardingRuleStatus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<RuleCondition>,

    /// Generation of the rule that the status is updated for.
    pub observed_generation: i64,

    /// The time when the status is last updated.
    pub observed_time: meta::v1::Time,

    /// Number of objects converted to this shard size.
    pub converted_objects: i64,

    /// Number of objects not yet converted to the sharding label for this size.
    #[serde(default, skip_serializing_if = "is_default")]
    pub unconverted_objects: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuleCondition {
    /// Type of the condition.
    pub ty:                   RuleConditionType,
    /// True or False describing the condition.
    pub status:               RuleConditionStatus,
    /// Unique, one-word, CamelCase reason for the condition's last transition.
    pub reason:               Option<String>,
    /// Human-readable message indicating details about last transition.
    pub message:              Option<String>,
    /// Last time the condition transitioned from one status to another.
    pub last_transition_time: Option<meta::v1::Time>,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleConditionType {
    /// Whether the rule is actively applied.
    Active,
}

#[derive(
    serde::Serialize, serde::Deserialize, Debug, Clone, schemars::JsonSchema, PartialEq, Eq,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleConditionStatus {
    True,
    False,
}

fn is_default<T: Default + Eq>(t: &T) -> bool { *t == T::default() }
