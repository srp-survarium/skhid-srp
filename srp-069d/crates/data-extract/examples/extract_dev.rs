//! Development run: build every table from a game folder into OUT, reporting each table's result.
//! usage: extract_dev <resources dir> <survarium.exe> <out dir>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let src = data_extract::Source::open(a[1].as_ref(), a[2].as_ref()).expect("open");
    let t0 = std::time::Instant::now();
    let out = std::path::Path::new(&a[3]);
    std::fs::create_dir_all(out).unwrap();
    let mut bad = 0;
    for (name, r) in data_extract::extract_each(&src) {
        match r {
            Ok(b) => std::fs::write(out.join(name), b).unwrap(),
            Err(e) => {
                bad += 1;
                let _ = std::fs::remove_file(out.join(name));
                println!("FAIL {name}: {e}");
            }
        }
    }
    println!("done in {} ms, {bad} failed", t0.elapsed().as_millis());
}
