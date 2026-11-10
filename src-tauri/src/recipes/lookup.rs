//! Match recipe identifiers against configurations already available locally.
use super::{Recipe, fingerprint::fingerprint};

struct Identifier {
    version: Option<u32>,
    hex: String,
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn parse(text: &str) -> Result<Identifier, String> {
    // Identifiers are metadata, not document payloads.
    if text.len() > 256 {
        return Err("Recipe identifier is too long.".into());
    }
    let mut lines = text.trim().lines();
    let first = lines.next().ok_or("Enter a recipe fingerprint.")?.trim();
    let (version, value) = if let Some(value) = first.strip_prefix("CRYPT-R1-") {
        (Some(1), value)
    } else if let Some(value) = first.strip_prefix("CRYPT-R2-") {
        (Some(2), value)
    } else if hex(first, 64) {
        (None, first)
    } else {
        return Err("Use a CRYPT-R1/CRYPT-R2 code or a full SHA-256 digest.".into());
    };
    if !hex(value, 16) && !hex(value, 64) {
        return Err("Recipe code must contain 16 or 64 hexadecimal digits.".into());
    }
    let value = value.to_ascii_lowercase();
    let full = if let Some(second) = lines.next() {
        let full = second
            .trim()
            .strip_prefix("SHA-256: ")
            .ok_or("Invalid fingerprint companion.")?;
        if version.is_none() || !hex(full, 64) || !full.to_ascii_lowercase().starts_with(&value) {
            return Err("Fingerprint code and full digest disagree.".into());
        }
        full.to_ascii_lowercase()
    } else {
        value
    };
    if lines.next().is_some() {
        return Err("Unexpected data after recipe fingerprint.".into());
    }
    Ok(Identifier { version, hex: full })
}

fn unique_match(
    matches: impl IntoIterator<Item = (String, Recipe)>,
) -> Result<Option<Recipe>, String> {
    let mut selected: Option<(String, Recipe)> = None;
    for (digest, recipe) in matches {
        if selected
            .as_ref()
            .is_some_and(|(existing, _)| *existing != digest)
        {
            return Err("This short code matches multiple configurations. Use the full digest or import recipe JSON.".into());
        }
        if selected.is_none() {
            selected = Some((digest, recipe));
        }
    }
    Ok(selected.map(|(_, recipe)| recipe))
}

/// Returns no match without guessing rules. Identical configurations with
/// different metadata count as one match; short-code collisions are errors.
pub fn lookup(identifier: &str, recipes: &[Recipe]) -> Result<Option<Recipe>, String> {
    let wanted = parse(identifier)?;
    let mut matches = Vec::new();
    for recipe in recipes {
        let fp = fingerprint(recipe)?;
        if wanted
            .version
            .is_none_or(|version| recipe.schema_version == version)
            && fp.value.starts_with(&wanted.hex)
        {
            matches.push((fp.value, recipe.clone()));
        }
    }
    unique_match(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::{Recipe, fingerprint};

    fn examples() -> Vec<Recipe> {
        let mut recipes = crate::recipes::builtins().unwrap();
        let mut copy = recipes[0].clone();
        copy.id = "user-renamed".into();
        copy.name = "Renamed example".into();
        recipes.push(copy);
        recipes
    }

    #[test]
    fn matches_full_short_and_companion_without_metadata_ambiguity() {
        let recipes = examples();
        let fp = fingerprint::fingerprint(&recipes[0]).unwrap();
        for identifier in [
            &fp.display,
            &fp.value,
            &format!("{}\nSHA-256: {}\n", fp.display, fp.value),
        ] {
            let matched = lookup(identifier, &recipes).unwrap().unwrap();
            assert_eq!(fingerprint::fingerprint(&matched).unwrap().value, fp.value);
        }
    }

    #[test]
    fn rejects_malformed_identifiers_and_disagreeing_companions() {
        let recipes = examples();
        let fp = fingerprint::fingerprint(&recipes[0]).unwrap();
        for identifier in [
            "",
            "CRYPT-R3-0123456789abcdef",
            "CRYPT-R1-broken",
            "CRYPT-R2-0123456789abcdeg",
        ] {
            assert!(lookup(identifier, &recipes).is_err());
        }
        assert!(
            lookup(
                &format!("{}\nSHA-256: {}", fp.display, "0".repeat(64)),
                &recipes
            )
            .is_err()
        );
        assert!(lookup(&"0".repeat(64), &recipes).unwrap().is_none());
    }

    #[test]
    fn a_short_collision_never_selects_by_catalog_order() {
        let recipes = examples();
        let mut same_prefix = "0123456789abcdef".to_owned();
        same_prefix.push_str(&"0".repeat(48));
        let other = format!("0123456789abcdef{}", "1".repeat(48));
        assert!(
            unique_match([
                (same_prefix.clone(), recipes[0].clone()),
                (other, recipes[1].clone())
            ])
            .is_err()
        );
        assert!(
            unique_match([
                (same_prefix.clone(), recipes[0].clone()),
                (same_prefix, recipes[1].clone())
            ])
            .unwrap()
            .is_some()
        );
    }
}
