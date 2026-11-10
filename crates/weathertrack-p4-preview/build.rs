fn main() {
    // the UI files are in another crate, so tell cargo to rebuild when they change
    println!("cargo:rerun-if-changed=../weathertrack-p4/ui");
    println!("cargo:rerun-if-changed=../weathertrack-view/slint");
    let cfg = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../weathertrack-p4/ui/app.slint", cfg).unwrap();
}
