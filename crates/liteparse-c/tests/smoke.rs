use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn profile_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    exe.parent()
        .and_then(Path::parent)
        .expect("target/<profile>/deps layout")
        .to_path_buf()
}

fn c_compiler() -> Option<String> {
    let candidates = std::env::var("CC")
        .map(|cc| vec![cc])
        .unwrap_or_else(|_| vec!["cc".into(), "clang".into(), "gcc".into()]);
    candidates.into_iter().find(|cc| {
        Command::new(cc)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
    })
}

fn shared_library(profile_dir: &Path) -> Option<PathBuf> {
    [
        "libliteparse_c.so",
        "libliteparse_c.dylib",
        "liteparse_c.dll",
    ]
    .iter()
    .map(|name| profile_dir.join("deps").join(name))
    .find(|path| path.exists())
}

#[test]
fn header_smoke_binary_passes_on_fixtures() {
    let required = std::env::var_os("LITEPARSE_REQUIRE_SMOKE").is_some();
    let Some(cc) = c_compiler() else {
        assert!(
            !required,
            "LITEPARSE_REQUIRE_SMOKE is set but no C compiler was found"
        );
        eprintln!("skipping: no C compiler found");
        return;
    };
    let profile_dir = profile_dir();
    let Some(library) = shared_library(&profile_dir) else {
        assert!(
            !required,
            "LITEPARSE_REQUIRE_SMOKE is set but no shared library was built"
        );
        eprintln!(
            "skipping: shared library not built in {}",
            profile_dir.display()
        );
        return;
    };

    let include = manifest_dir().join("include");
    let source = manifest_dir().join("tests/header_smoke.c");
    let library_dir = library.parent().expect("library directory");
    let binary = profile_dir.join("liteparse_header_smoke");
    let compile = Command::new(&cc)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-pedantic"])
        .arg("-I")
        .arg(&include)
        .arg(&source)
        .arg("-L")
        .arg(library_dir)
        .arg(format!("-Wl,-rpath,{}", library_dir.display()))
        .args(["-lliteparse_c", "-lm", "-o"])
        .arg(&binary)
        .output()
        .expect("run the C compiler");
    assert!(
        compile.status.success(),
        "compiling header_smoke.c against {} failed:\n{}",
        library.display(),
        String::from_utf8_lossy(&compile.stderr)
    );

    let fixtures = manifest_dir().join("../../integration_tests_data");
    // Avoid an uplifted library left by an earlier `cargo build`.
    let run = Command::new(&binary)
        .env("LD_LIBRARY_PATH", library_dir)
        .env("DYLD_LIBRARY_PATH", library_dir)
        .arg(fixtures.join("sample.pdf"))
        .arg(fixtures.join("receipt.png"))
        .output()
        .expect("run the smoke binary");
    assert!(
        run.status.success(),
        "header_smoke failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}
