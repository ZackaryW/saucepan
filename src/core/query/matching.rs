use super::model::{Condition, Operator};
use crate::{
    models::index::{ClaimContext, validate_context_field},
    utils::pattern::StringGlob,
};
use anyhow::{Context, Result, ensure};

enum Matcher {
    Eq(String),
    Glob(StringGlob),
}
pub(super) struct CompiledConditions(Vec<(String, Matcher)>);
impl CompiledConditions {
    pub(super) fn compile(conditions: &[Condition]) -> Result<Self> {
        ensure!(conditions.len() <= 64, "query exceeds 64 conditions");
        let mut compiled = Vec::with_capacity(conditions.len());
        for (index, condition) in conditions.iter().enumerate() {
            let label = || format!("condition {index} ({:?})", condition.key);
            validate_context_field(&condition.key, &condition.value).with_context(label)?;
            let matcher = match condition.op {
                Operator::Eq => Matcher::Eq(condition.value.clone()),
                Operator::Glob => {
                    Matcher::Glob(StringGlob::compile(&condition.value).with_context(label)?)
                }
            };
            compiled.push((condition.key.clone(), matcher));
        }
        Ok(Self(compiled))
    }
    pub(super) fn matches(&self, context: &ClaimContext) -> bool {
        self.0.iter().all(|(key, matcher)| {
            context.get(key).is_some_and(|value| match matcher {
                Matcher::Eq(expected) => expected == value,
                Matcher::Glob(pattern) => pattern.is_match(value),
            })
        })
    }
}
