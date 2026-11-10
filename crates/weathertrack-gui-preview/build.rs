fn main() {
    let cfg = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer);
    slint_build::compile_with_config("../weathertrack-gui/ui/app.slint", cfg).unwrap();
}
