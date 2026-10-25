//! A 0.46c2 / 0.69d binary config (16-byte value records) written again in 0.26g's layout
//! (24-byte records), the same tree: `widen-config [--particle] <in> <out> [<in> <out> ...]`.
//! Built by prepare_donor.sh against resource-porter's BinaryConfig (its reader takes both
//! layouts, its writer writes 0.26g's).
//!
//! `--particle`: a particle's TargetColor actions (`particle_action_target_color_type`, type
//! index 14: 0.26g's particle loader, 0x364F1C, has no case for it and leaves the action out)
//! become InitColor properties (type index 4) of the same colour source: every one of the
//! attachments' effects gives TargetColor one constant colour.
use vostok_resource_porter::binary_config::{BinaryConfig, ConfigValue, NamedValue};

fn get<'a>(entries: &'a [NamedValue], name: &str) -> Option<&'a ConfigValue> {
    entries.iter().find(|e| e.name == name).map(|e| &e.value)
}

fn set(entries: &mut [NamedValue], name: &str, value: ConfigValue) -> anyhow::Result<()> {
    let entry = entries
        .iter_mut()
        .find(|e| e.name == name)
        .ok_or_else(|| anyhow::anyhow!("a TargetColor action without {name}"))?;
    entry.value = value;
    Ok(())
}

fn target_color_to_init_color(value: &mut ConfigValue) -> anyhow::Result<usize> {
    let mut count = 0;
    match value {
        ConfigValue::Named(entries) => {
            let kind = get(entries, "particle_entity_type");
            if kind == Some(&ConfigValue::String("particle_action_target_color_type".into())) {
                set(entries, "particle_entity_type", ConfigValue::String("particle_action_initial_color_type".into()))?;
                set(entries, "type_index", ConfigValue::Integer(4))?;
                set(entries, "node_config_field", ConfigValue::String("InitColor".into()))?;
                set(entries, "type", ConfigValue::String("property".into()))?;
                let Some(ConfigValue::Named(properties)) =
                    entries.iter_mut().find(|e| e.name == "properties").map(|e| &mut e.value)
                else {
                    anyhow::bail!("a TargetColor action without properties");
                };
                properties.retain(|p| p.name != "Delay" && p.name != "Duration");
                set(properties, "Name", ConfigValue::String("InitColor".into()))?;
                let source = properties
                    .iter_mut()
                    .find(|p| p.name == "TargetColor")
                    .ok_or_else(|| anyhow::anyhow!("a TargetColor action without its colour"))?;
                source.name = "InitColor".into();
                count += 1;
            }
            for entry in entries.iter_mut() {
                count += target_color_to_init_color(&mut entry.value)?;
            }
        }
        ConfigValue::Indexed(items) => {
            for item in items.iter_mut() {
                count += target_color_to_init_color(item)?;
            }
        }
        _ => (),
    }
    Ok(count)
}

fn main() -> anyhow::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let particle = args.first().is_some_and(|a| a == "--particle");
    if particle {
        args.remove(0);
    }
    if args.is_empty() || args.len() % 2 == 1 {
        anyhow::bail!("usage: widen-config [--particle] <in> <out> [<in> <out> ...]");
    }
    for pair in args.chunks(2) {
        let bytes = std::fs::read(&pair[0])?;
        let mut config = BinaryConfig::parse(&bytes)?;
        if particle {
            let n = target_color_to_init_color(&mut config.root)?;
            eprintln!("{}: {n} TargetColor actions made InitColor", pair[1]);
        }
        std::fs::write(&pair[1], config.to_bytes()?)?;
    }
    Ok(())
}
