//! Small editable notebook inputs share schema-2 transform semantics. Full
//! document jobs use disk artifacts and bounded previews instead of strings.
use super::{Control, stream::io_error, transform};
use crate::{
    engine::recovery::WordChoice,
    recipes::{self, Recipe, RecipeResult, Rule, StageOutput, fingerprint},
};
use std::fs;

pub(crate) fn evaluate(
    recipe: &Recipe,
    text: &str,
    key: &str,
    undo: bool,
    choices: &[WordChoice],
) -> Result<RecipeResult, String> {
    recipe.validate()?;
    if text.len() > recipes::MAX_INPUT_BYTES {
        return Err("Editable notebook input supports up to 16 KiB. Open the text as a file to process larger documents.".into());
    }
    let key = recipes::normalize_key(recipe, key)?;
    if !undo && !choices.is_empty() {
        return Err("Cannot select plaintext candidates while encoding.".into());
    }
    let root = super::private_tempdir("cryptoplay-notebook-", None)
        .map_err(|e| io_error("Cannot create notebook artifacts", e))?;
    let mut current = root.path().join("source");
    fs::write(&current, text).map_err(|e| io_error("Cannot snapshot notebook text", e))?;
    let mut steps: Vec<_> = recipe
        .steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.enabled)
        .collect();
    let exact = steps.iter().all(|(_, step)| !step.rule.lossy());
    if undo {
        steps.reverse();
    }
    let mut stages = Vec::new();
    for (index, step) in steps {
        if undo && step.rule.lossy() {
            break;
        }
        let resolved = fingerprint::resolve(recipe, index, &step.rule, &key)?;
        let rule = resolved.as_ref().map_or(&step.rule, |(rule, _)| rule);
        let destination = root.path().join(format!("stage-{index}"));
        transform::transform(
            rule,
            &current,
            &destination,
            &key,
            undo,
            &Control::default(),
        )?;
        let label = resolved.map_or_else(|| step.rule.name().into(), |(_, label)| label);
        let text = fs::read_to_string(&destination)
            .map_err(|e| io_error("Cannot read notebook stage", e))?;
        stages.push(StageOutput {
            label: if undo { format!("Undo {label}") } else { label },
            text,
            lossy: step.rule.lossy(),
        });
        current = destination;
    }
    let intermediate =
        fs::read_to_string(current).map_err(|e| io_error("Cannot read notebook result", e))?;
    if !undo || exact {
        if !choices.is_empty() {
            return Err("Reversible recipes have no ambiguous word selections.".into());
        }
        return Ok(RecipeResult {
            output: intermediate.clone(),
            intermediate,
            exact,
            reconstruction: None,
            note: if exact {
                "Reversible Unicode recipe: decoding restores the input exactly.".into()
            } else {
                "Lossy recipe: original information is permanently discarded.".into()
            },
            stages,
        });
    }
    let rules: Vec<_> = recipe
        .steps
        .iter()
        .filter(|step| step.enabled)
        .map(|step| &step.rule)
        .collect();
    let reconstruction = match rules.as_slice() {
        [Rule::PhoneticRotate, suffix @ ..] if suffix.iter().all(|rule| !rule.lossy()) => {
            if choices.is_empty() {
                Some(crate::engine::recovery::reconstruct(&intermediate, &key))
            } else {
                Some(crate::engine::recovery::reconstruct_with_choices(
                    &intermediate,
                    &key,
                    choices,
                )?)
            }
        }
        [Rule::RemoveVowels, Rule::CollapseDoubles, suffix @ ..]
            if suffix.iter().all(|rule| !rule.lossy()) =>
        {
            Some(recipes::skeleton::reconstruct(&intermediate, choices)?)
        }
        _ => {
            if !choices.is_empty() {
                return Err(
                    "This lossy recipe has no compatible English reconstruction strategy.".into(),
                );
            }
            None
        }
    };
    let output = reconstruction
        .as_ref()
        .and_then(|r| r.suggested.clone())
        .unwrap_or_default();
    let note = reconstruction.as_ref().map_or_else(
        || {
            "Exact surviving text restored; no English reconstruction strategy for this recipe."
                .into()
        },
        |r| r.note.clone(),
    );
    Ok(RecipeResult {
        output,
        intermediate,
        exact: false,
        reconstruction,
        note,
        stages,
    })
}
