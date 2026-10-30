fn main() {
    // Dialogs, string tables, manifest and version info; a no-op off the MSVC target.
    embed_resource::compile("res/installer.rc", embed_resource::NONE);
    println!("cargo:rerun-if-changed=res/installer.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
}
