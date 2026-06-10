fn main() {
    println!("cargo:rerun-if-changed=sonic/sonic.c");
    println!("cargo:rerun-if-changed=sonic/sonic.h");

    cc::Build::new()
        .file("sonic/sonic.c")
        .include("sonic")
        .warnings(false)
        .compile("voicesub_sonic");
}
