use std::path::PathBuf;

fn main() {
    let src_dir = PathBuf::from("src");

    let mut build = cc::Build::new();
    build.include(&src_dir);
    build.flag_if_supported("-Wno-unused-parameter");
    build.flag_if_supported("-Wno-unused-but-set-variable");
    build.flag_if_supported("-Wno-trigraphs");

    build.file(src_dir.join("parser.c"));
    build.compile("tree-sitter-swede");

    println!("cargo:rerun-if-changed=src/parser.c");
}
