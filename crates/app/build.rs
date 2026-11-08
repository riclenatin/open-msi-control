fn main() {
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.manifest.xml");
    println!("cargo:rerun-if-changed=resources/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // MSI_ACPI is reachable only by Administrators, so the manifest requests elevation.
        // Linked into the app binary only, so `cargo test` harnesses run unelevated.
        // gpui's own `windows-manifest` feature is disabled to avoid a duplicate manifest.
        embed_resource::compile_for("resources/app.rc", ["open-msi-control"], embed_resource::NONE)
            .manifest_required()
            .expect("embedding Windows resources");
    }
}
