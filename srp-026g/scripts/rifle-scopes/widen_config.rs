//! A 0.69d0 binary config (16-byte value records) written again in 0.26g's layout (24-byte
//! records), the same tree: `widen-config <in> <out> [<in> <out> ...]`. Built by prepare_donor.sh
//! against resource-porter's BinaryConfig (its reader takes both layouts, its writer writes 0.26g's).
use vostok_resource_porter::binary_config::BinaryConfig;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.len() % 2 == 1 {
        anyhow::bail!("usage: widen-config <in> <out> [<in> <out> ...]");
    }
    for pair in args.chunks(2) {
        let config = BinaryConfig::parse(&std::fs::read(&pair[0])?)?;
        std::fs::write(&pair[1], config.to_bytes()?)?;
    }
    Ok(())
}
