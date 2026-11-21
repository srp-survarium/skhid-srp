//! `srp-check-scope-fits <released game data> <candidate game data>`: no
//! scope pair players could put on is lost by the candidate.
//!
//! The released data (up to revision 68) is read as its servers read it:
//! the scope variants' `srp_scope_fit` blocks
//! (`survarium::v026g::scopes::released`). The candidate is asked what its
//! servers will ask: its modules table (`weapon_modules::Modules`), with
//! the same `check` an attach and an account's load run. Every released
//! `(weapon, scope)` pair must still be put on in the candidate; each one
//! that is not is listed, and the exit status is 1.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

use survarium::v026g::{
    dictionary::Dictionary,
    game_data::{self, GameData},
    scopes::{self, released},
    weapon_modules::Modules,
};

fn unique_modification<'a>(
    owners: &mut BTreeMap<i32, &'a str>,
    id: i32,
    path: &'a str,
) -> Result<(), String> {
    if let Some(previous) = owners.insert(id, path) {
        return Err(format!(
            "item modification {id} is defined by both {previous} and {path}; the native global lookup can apply the wrong modifiers"
        ));
    }
    Ok(())
}

// The client collects modifications from every item, not just category 22:
// a scope's own modification (which names it to the weapon) can otherwise
// resolve to another item's.
fn check_modification_ids(data: &GameData, dictionary: &Dictionary) -> Result<(), Box<dyn Error>> {
    let mut owners = BTreeMap::new();
    let mut paths = BTreeSet::new();
    for item in &dictionary.items {
        let path = item.cfg_name.as_str();
        if !paths.insert(path) {
            continue;
        }
        let config =
            game_data::config(data, path)?.ok_or_else(|| format!("missing item config {path}"))?;
        if let Some(modifications) = config.get("modifications") {
            for modification in modifications.children() {
                unique_modification(&mut owners, modification.i32_at("id")?, path)?;
            }
        }
    }
    Ok(())
}

fn open(path: &Path) -> Result<(GameData, Dictionary), Box<dyn Error>> {
    let data = GameData::open(path)?;
    let bytes = data
        .file("gameplay/db_static_dictionaries")
        .ok_or("game data has no dictionary")?;
    let dictionary = Dictionary::parse(bytes)?;
    Ok((data, dictionary))
}

fn run(before: &Path, after: &Path) -> Result<(), Box<dyn Error>> {
    let (old_data, old_dictionary) = open(before)?;
    let (old, rejected) = released::pairs(&old_data, &old_dictionary)?;
    if !rejected.is_empty() {
        return Err(format!(
            "{}: rejected scope variants: {rejected:?}",
            before.display()
        )
        .into());
    }
    let (data, dictionary) = open(after)?;
    check_modification_ids(&data, &dictionary)?;
    scopes::check(&data, &dictionary)?;
    let modules = Modules::of(&data).map_err(|why| format!("the modules table: {why:?}"))?;
    let candidate = released::Candidate {
        dictionary: &dictionary,
        modules: &modules,
    };
    let name = |dictionary: &Dictionary, id: u16| {
        dictionary
            .item(id)
            .map_or_else(|| "?".to_owned(), |item| item.name.clone())
    };
    let mut losses = Vec::new();
    for pair in &old {
        let weapon = (pair.weapon, name(&old_dictionary, pair.weapon));
        let scope = (pair.scope, name(&old_dictionary, pair.scope));
        if let Some(why) = candidate.lost((weapon.0, &weapon.1), (scope.0, &scope.1)) {
            losses.push(format!(
                "lost weapon {} / scope {} (variant {}): {why}",
                pair.weapon, pair.scope, pair.variant
            ));
        }
    }
    let new = scopes::pairs(&dictionary, &modules);
    // The lobby's window greys out a scope its weapon has no
    // `items_compatibility` row with: a pair the table has and the
    // dictionary lacks could never be put on.
    for &(weapon, scope) in &new {
        if !dictionary.compatible(weapon, scope) {
            losses.push(format!(
                "weapon {weapon} / scope {scope} is absent from lobby compatibility"
            ));
        }
    }
    if !losses.is_empty() {
        for loss in &losses {
            eprintln!("{loss}");
        }
        return Err(format!("{} pair(s) refused", losses.len()).into());
    }
    let released: BTreeSet<(u16, u16)> = old.iter().map(|pair| (pair.weapon, pair.scope)).collect();
    println!("status\tweapon\tscope\tweapon_name\tscope_name");
    for &(weapon, scope) in &new {
        let status = if released.contains(&(weapon, scope)) {
            "preserved"
        } else {
            "added"
        };
        println!(
            "{status}\t{weapon}\t{scope}\t{}\t{}",
            name(&dictionary, weapon),
            name(&dictionary, scope)
        );
    }
    let weapons: BTreeSet<u16> = new.iter().map(|&(weapon, _)| weapon).collect();
    let sold: BTreeSet<u16> = new.iter().map(|&(_, scope)| scope).collect();
    let added = new.iter().filter(|pair| !released.contains(pair)).count();
    eprintln!(
        "{} pairs preserved; {added} added; {} scopes on {} weapons",
        old.len(),
        sold.len(),
        weapons.len()
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

    #[test]
    fn two_items_cannot_share_a_modification_id() {
        for paths in [
            ["shop_scope_docter", "srp_muzzle_zenit_dtk3"],
            ["srp_muzzle_zenit_dtk3", "shop_scope_docter"],
        ] {
            let mut owners = BTreeMap::new();
            unique_modification(&mut owners, 993111, paths[0]).unwrap();
            let error = unique_modification(&mut owners, 993111, paths[1]).unwrap_err();
            assert!(error.contains(paths[0]) && error.contains(paths[1]));
        }
        let mut owners = BTreeMap::new();
        unique_modification(&mut owners, 993111, "shop_scope_docter").unwrap();
        unique_modification(&mut owners, 1003111, "srp_muzzle_zenit_dtk3").unwrap();
    }
}
