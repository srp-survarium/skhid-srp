//! The scope pairs released up to revision 68, for the preservation check
//! alone (`srp-check-scope-fits`): no server reads them.
//!
//! Until the scopes moved onto the modules table, a pair was a *variant*: a
//! category-22 entry made for one weapon, whose config named its shop scope
//! and weapons (`srp_scope_fit`), and an upgrade record of the scope held
//! the variant's dict id. Every such pair is one a player may have. They are
//! kept in `scripts/scope-fits/released-pairs.tsv`, made once from release
//! 68's files (`released.py`; the same pairs and variants as the servers of
//! those releases read) and never again: [`parse`] reads it.

use super::super::dictionary::Dictionary;
use super::super::weapon_modules::{Fitted, Modules, Unfit};
use super::SCOPE_CATEGORY;

/// One released pair: the variant a weapon showed a scope item by, each
/// item with its dict id and its name in release 68.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Released {
    pub weapon: (u16, String),
    /// The scope item: what the player bought and owns.
    pub scope: (u16, String),
    /// The variant's dict id: what a record of the scope on the weapon held.
    pub variant: u16,
}

/// The pairs of `released-pairs.tsv`: one a line, `weapon`, its dict id,
/// `scope`, its dict id, the variant's dict id and its config; `#` lines
/// are comments. A line that does not read is an error naming it.
pub fn parse(tsv: &str) -> Result<Vec<Released>, String> {
    let id = |field: &str, line: usize| {
        field
            .parse::<u16>()
            .map_err(|_| format!("line {line}: `{field}` is no dict id"))
    };
    tsv.lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty() && !line.starts_with('#'))
        .map(|(index, line)| {
            let number = index + 1;
            let fields: Vec<&str> = line.split('\t').collect();
            let [weapon, weapon_id, scope, scope_id, variant, _config] = fields[..] else {
                return Err(format!("line {number}: {} fields, not 6", fields.len()));
            };
            Ok(Released {
                weapon: (id(weapon_id, number)?, weapon.to_owned()),
                scope: (id(scope_id, number)?, scope.to_owned()),
                variant: id(variant, number)?,
            })
        })
        .collect()
}

/// What a candidate game data's servers answer about scopes: its
/// dictionary and its modules table.
pub struct Candidate<'a> {
    pub dictionary: &'a Dictionary,
    pub modules: &'a Modules,
}

impl Candidate<'_> {
    /// Why the released pair can no longer be put on, if it can not: an
    /// item gone or under another name, a scope that is no scope item the
    /// shop sells any more, or the refusal of the one fit check
    /// ([`Modules::check`], what an attach and an account's load run).
    pub fn lost(&self, pair: &Released) -> Option<String> {
        for ((id, name), what) in [(&pair.weapon, "weapon"), (&pair.scope, "scope")] {
            match self.dictionary.item(*id) {
                None => return Some(format!("{what} {id} `{name}` is gone")),
                Some(item) if item.name != *name => {
                    return Some(format!("{what} {id} `{name}` is now `{}`", item.name));
                }
                Some(_) => {}
            }
        }
        let ((weapon, weapon_name), (scope, scope_name)) = (&pair.weapon, &pair.scope);
        // (`super::super::prices::sellers_of_with`'s rule for a module.)
        let sold = self.dictionary.item(*scope).is_some_and(|item| {
            item.category == SCOPE_CATEGORY && !item.is_premium && item.base_cost != 0
        });
        if !sold || !self.modules.is_module(*scope) {
            return Some(format!("scope {scope} `{scope_name}` is not sold"));
        }
        let fitted = Fitted {
            scope: Some(*scope),
            ..Fitted::default()
        };
        match self.modules.check(*weapon, fitted) {
            Ok(()) => None,
            Err(Unfit::NoGun) => Some(format!(
                "weapon {weapon} `{weapon_name}` is no gun of the modules table"
            )),
            Err(Unfit::Refused(refusal)) => Some(format!(
                "the modules table refuses scope {scope} `{scope_name}` on weapon {weapon} \
                 `{weapon_name}`: {refusal:?}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::weapon_modules::Kind;
    use super::super::fixtures::{self, COBRA, LONE_SCOPE, PSO_1};
    use super::*;

    #[test]
    fn the_record_is_read_line_by_line_and_a_broken_line_is_named() {
        let tsv = "# a comment\n# weapon\t...\n\
                   ak_74m\t3\tshop_scope_pso_1\t2101\t2006\tgameplay/items/scopes/fit_ak_74m_pso_1.options\n";
        assert_eq!(
            parse(tsv).unwrap(),
            [Released {
                weapon: (3, "ak_74m".into()),
                scope: (2101, "shop_scope_pso_1".into()),
                variant: 2006,
            }]
        );
        let why = parse("ak_74m\t3\tshop_scope_pso_1\t2101\t2006\n").unwrap_err();
        assert!(why.contains("line 1") && why.contains("5 fields"), "{why}");
        let why = parse("#\nak_74m\tx\tshop_scope_pso_1\t2101\t2006\tc\n").unwrap_err();
        assert!(why.contains("line 2") && why.contains("`x`"), "{why}");
    }

    /// The fixture's candidate (the AK-74N and its premium copy take the
    /// PSO-1 and the Cobra, the SVD the Cobra), or one with `table`.
    fn with_candidate<T>(table: Option<&Modules>, ask: impl FnOnce(&Candidate<'_>) -> T) -> T {
        let dictionary = fixtures::dictionary_of(&fixtures::with_scopes());
        ask(&Candidate {
            dictionary: &dictionary,
            modules: table.unwrap_or(fixtures::modules()),
        })
    }

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    fn pair(weapon: &str, (scope, scope_name): (u16, &str)) -> Released {
        Released {
            weapon: (dict(weapon), weapon.into()),
            scope: (scope, scope_name.into()),
            variant: 0,
        }
    }

    const PSO: (u16, &str) = (PSO_1, "shop_scope_pso_1");
    const EKP: (u16, &str) = (COBRA, "shop_scope_cobra_ekp_8");

    #[test]
    fn a_released_pair_the_table_fits_is_preserved() {
        with_candidate(None, |candidate| {
            assert_eq!(candidate.lost(&pair("ak_74n", PSO)), None);
            assert_eq!(candidate.lost(&pair("svd", EKP)), None);
        });
    }

    #[test]
    fn a_pair_the_table_does_not_fit_is_lost_whatever_else_it_gained() {
        let (ak, svd, ak_74m) = (dict("ak_74n"), dict("svd"), dict("ak_74m"));
        with_candidate(None, |candidate| {
            // the gun takes scopes, not this one
            let why = candidate.lost(&pair("svd", PSO)).unwrap();
            assert!(why.contains("refuses scope 2101") && why.contains("NoFit"), "{why}");
            // no gun of the table at all
            let why = candidate.lost(&pair("ak_74m", PSO)).unwrap();
            assert!(why.contains("is no gun of the modules table"), "{why}");
            // a scope item nothing takes is not sold
            let why = candidate.lost(&pair("ak_74n", (LONE_SCOPE, "shop_scope_lone"))).unwrap();
            assert!(why.contains("is not sold"), "{why}");
        });
        // a table with more guns and pairs that drops one released pair
        let grown = super::super::super::weapon_modules::fixtures::modules(&[
            (ak, COBRA, Kind::Scope),
            (svd, COBRA, Kind::Scope),
            (svd, PSO_1, Kind::Scope),
            (ak_74m, PSO_1, Kind::Scope),
        ]);
        with_candidate(Some(&grown), |candidate| {
            assert!(candidate.lost(&pair("ak_74n", PSO)).is_some());
            assert_eq!(candidate.lost(&pair("ak_74n", EKP)), None);
        });
    }

    #[test]
    fn an_item_gone_or_under_another_name_loses_its_pairs() {
        with_candidate(None, |candidate| {
            let why = candidate.lost(&pair("ak_74n", (2198, "shop_scope_gone"))).unwrap();
            assert!(why.contains("scope 2198 `shop_scope_gone` is gone"), "{why}");
            let renamed = Released {
                weapon: (dict("ak_74n"), "ak_47".into()),
                ..pair("ak_74n", PSO)
            };
            let why = candidate.lost(&renamed).unwrap();
            assert!(why.contains("weapon") && why.contains("is now `ak_74n`"), "{why}");
        });
    }
}
