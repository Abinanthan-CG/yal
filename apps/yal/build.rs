fn main() {
    println!("cargo:rerun-if-changed=windows/yal.rc");
    println!("cargo:rerun-if-changed=windows/yal.manifest");
    println!("cargo:rerun-if-changed=../../assets/yal.ico");
    embed_resource::compile("windows/yal.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to compile Windows resources (icon + manifest)");
}
