//! `cargo run -p weapon-modules --example preview_eyes -- <weapon_modules.srpm> <gun id> <scope model part>...`
//!
//! A gun's bare model, its bind pose's bones, and each named fitted scope's parts,
//! optic, frame and eye (with its relief along the eye line) as this crate resolves
//! them: JSON for scripts/weapon-view-preview/layers.py (the Weapon view window's
//! preview, patch 218). With no scope named, it lists the gun's fits on stderr.
use weapon_modules::{Affine, BoneIndex, DictId, EyeAnchor, Kind, Loadout, Placement, Relief, Role, Table, Wanted, resolve};

fn affine(a: &Affine) -> String {
    format!("{:?}", a.floats())
}

fn place(p: &Placement) -> String {
    format!("{{\"bone\": {}, \"offset\": {}}}", p.bone.0, affine(&p.offset))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let [_, table, gun, scopes @ ..] = args.as_slice() else {
        return Err("preview_eyes <weapon_modules.srpm> <gun id> <scope model part>...".into());
    };
    let bytes = std::fs::read(table).map_err(|e| format!("{table}: {e}"))?;
    let table = Table::parse(&bytes).map_err(|e| format!("{e:?}"))?;
    let id = gun.parse().ok().and_then(DictId::new).ok_or("a gun id is a dictionary id")?;
    let gun = table.gun(id).ok_or("no such gun in the table")?;
    if scopes.is_empty() {
        for m in gun.fits() {
            eprintln!("{:?} {}", m.role(), m.model().to_string_lossy());
        }
    }
    let poses: Vec<String> = (0..gun.bones())
        .filter_map(|b| gun.pose(BoneIndex(b)).map(|p| format!("\"{b}\": {}", affine(&p))))
        .collect();
    let mut fits = Vec::new();
    for name in scopes {
        let module = gun
            .fits()
            .find(|m| m.role() == Role::Fitted(Kind::Scope) && m.model().to_string_lossy().contains(name.as_str()))
            .ok_or(format!("no scope {name} fits the gun"))?;
        let Ok(Loadout::Bare(bare)) = resolve(gun, Wanted { scope: Some(module.dict()), ..Wanted::default() }) else {
            return Err(format!("{name}: not a bare loadout"));
        };
        let (Some(optic), Some(eye)) = (bare.optic, bare.eye) else { return Err(format!("{name}: no optic or eye")) };
        let parts: Vec<String> = bare
            .parts()
            .map(|p| format!("{{\"model\": {:?}, \"at\": {}}}", p.model().to_string_lossy(), place(&p.at)))
            .collect();
        let along = match eye.relief() {
            Relief::InGunPlane { along } => along,
            Relief::Own => 0.0,
        };
        fits.push(format!(
            "{name:?}: {{\"parts\": [{}], \"optic\": {:?}, \"aimed\": {:?}, \"frame\": {}, \"eye\": {}, \"own\": {}, \"along\": {along}}}",
            parts.join(", "),
            optic.module.model().to_string_lossy(),
            optic.module.aimed().map_or(String::new(), |a| a.to_string_lossy().into_owned()),
            place(&optic.frame),
            place(&eye.placed(EyeAnchor::GunIdle)),
            matches!(eye.relief(), Relief::Own),
        ));
    }
    println!(
        "{{\"bare\": {:?}, \"bones\": {}, \"own_aim\": {}, \"poses\": {{{}}}, \"fits\": {{{}}}}}",
        gun.bare().to_string_lossy(),
        gun.bones(),
        place(&gun.own_aim()),
        poses.join(", "),
        fits.join(", ")
    );
    Ok(())
}
