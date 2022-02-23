use ide_ci::env::expect_var;
use ide_ci::prelude::*;

fn main() -> Result {
    println!("cargo:rerun-if-changed=ide-paths.yaml");
    let yaml_contents = include_bytes!("ide-paths.yaml");
    let code = ide_ci::paths::process(yaml_contents.as_slice())?;
    let out_dir = expect_var("OUT_DIR")?.parse2::<PathBuf>()?;
    let out_path = out_dir.join("paths.rs");
    std::fs::write(out_path, code)?;
    Ok(())
}
