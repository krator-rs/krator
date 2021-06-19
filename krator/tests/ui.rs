use std::path::PathBuf;

#[cfg(target_os = "macos")]
fn link_deps(config: &mut compiletest_rs::Config) {
    let target_dir: &str = std::env!("CARGO_MANIFEST_DIR");
    let mut flags = config.target_rustcflags.take().unwrap_or_else(String::new);
    flags += " -L ";
    flags += &format!("{}/../target/debug", target_dir);
    flags += " -L ";
    flags += &format!("{}/../target/debug/deps", target_dir);
    eprintln!("{:?}", flags);
    config.target_rustcflags = Some(flags);
}

#[cfg(not(target_os = "macos"))]
fn link_deps(config: &mut compiletest_rs::Config) {
    config.link_deps();
}

#[test]
fn compile_test() {
    let mut config = compiletest_rs::Config {
        mode: compiletest_rs::common::Mode::Ui,
        src_base: PathBuf::from("tests/ui"),
        ..Default::default()
    };
    link_deps(&mut config);

    config.clean_rmeta();
    compiletest_rs::run_tests(&config);
}
