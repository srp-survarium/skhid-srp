//! Compare released and candidate scope catalogs using the server's own parser.
//! Refuse lost pairs, changed variant identities, and invisible lobby fits.

use std::{error::Error, path::Path};

use survarium::v026g::{dictionary::Dictionary, game_data::GameData, scopes::ScopeFits};

fn read(path: &Path) -> Result<(Dictionary, ScopeFits), Box<dyn Error>> {
    let data = GameData::open(path)?;
    let bytes = data
        .file("gameplay/db_static_dictionaries")
        .ok_or("game data has no dictionary")?;
    let dictionary = Dictionary::parse(bytes)?;
    let (fits, rejected) = ScopeFits::derive(&data, &dictionary)?;
    if !rejected.is_empty() {
        return Err(format!("{}: rejected scope variants: {rejected:?}", path.display()).into());
    }
    for fit in fits.all() {
        if !dictionary.compatible(fit.weapon, fit.scope) {
            return Err(format!(
                "{}: weapon {} / scope {} is absent from lobby compatibility",
                path.display(),
                fit.weapon,
                fit.scope
            )
            .into());
        }
    }
    Ok((dictionary, fits))
}

fn preserved(before: &ScopeFits, after: &ScopeFits) -> Result<(), String> {
    for fit in before.all() {
        match after.variant(fit.weapon, fit.scope) {
            Some(variant) if variant == fit.variant => {}
            Some(variant) => {
                return Err(format!(
                    "weapon {} / scope {} changed variant {} -> {variant}; saved attachments must survive",
                    fit.weapon, fit.scope, fit.variant
                ));
            }
            None => {
                return Err(format!(
                    "lost weapon {} / scope {} (variant {})",
                    fit.weapon, fit.scope, fit.variant
                ));
            }
        }
    }
    Ok(())
}

fn run(before: &Path, after: &Path) -> Result<(), Box<dyn Error>> {
    let (old_dictionary, old) = read(before)?;
    let (dictionary, new) = read(after)?;
    preserved(&old, &new)?;
    for id in old
        .scopes()
        .into_iter()
        .chain(old.weapons())
        .chain(old.all().iter().map(|f| f.variant))
    {
        let previous = old_dictionary
            .item(id)
            .ok_or("baseline fit names an absent item")?;
        let current = dictionary
            .item(id)
            .ok_or("candidate lost an existing item")?;
        if previous.name != current.name || previous.category != current.category {
            return Err(format!(
                "dictionary id {id} was repurposed: {} -> {}",
                previous.name, current.name
            )
            .into());
        }
    }
    println!("status\tweapon\tscope\tvariant\tweapon_name\tscope_name");
    for fit in new.all() {
        let status = if old.variant(fit.weapon, fit.scope).is_some() {
            "preserved"
        } else {
            "added"
        };
        let weapon = dictionary
            .item(fit.weapon)
            .ok_or("fit names an absent weapon")?;
        let scope = dictionary
            .item(fit.scope)
            .ok_or("fit names an absent scope")?;
        println!(
            "{status}\t{}\t{}\t{}\t{}\t{}",
            fit.weapon, fit.scope, fit.variant, weapon.name, scope.name
        );
    }
    eprintln!(
        "{} pairs preserved; {} added; {} scopes on {} weapons",
        old.all().len(),
        new.all().len() - old.all().len(),
        new.scopes().len(),
        new.weapons().len()
    );
    Ok(())
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [before, after] = args.as_slice() else {
        eprintln!(
            "usage: srp-check-scope-fits <released-server-game-data.db> <candidate-server-game-data.db>"
        );
        std::process::exit(2);
    };
    if let Err(error) = run(Path::new(before), Path::new(after)) {
        eprintln!("scope preservation check: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::scopes::Fit;

    fn catalog(pairs: &[(u16, u16, u16)]) -> ScopeFits {
        let (fits, duplicate) = ScopeFits::new(
            pairs
                .iter()
                .map(|&(weapon, scope, variant)| Fit {
                    weapon,
                    scope,
                    variant,
                })
                .collect(),
        );
        assert!(duplicate.is_empty());
        fits
    }

    #[test]
    fn more_scopes_and_more_weapons_keep_saved_attachments() {
        let old = catalog(&[(3, 2101, 2001), (4, 2101, 2002)]);
        let new = catalog(&[
            (3, 2101, 2001),
            (4, 2101, 2002),
            (3, 2113, 2701),
            (2283, 2113, 2901),
        ]);
        assert_eq!(preserved(&old, &new), Ok(()));
    }

    #[test]
    fn a_larger_catalog_can_still_lose_an_existing_pair() {
        let old = catalog(&[(3, 2101, 2001)]);
        let new = catalog(&[(4, 2101, 2002), (3, 2113, 2701)]);
        assert!(
            preserved(&old, &new)
                .unwrap_err()
                .contains("lost weapon 3 / scope 2101")
        );
    }

    #[test]
    fn changing_a_variant_would_strip_saved_attachments() {
        let old = catalog(&[(3, 2101, 2001)]);
        let new = catalog(&[(3, 2101, 2701)]);
        assert!(
            preserved(&old, &new)
                .unwrap_err()
                .contains("changed variant 2001 -> 2701")
        );
    }
}
