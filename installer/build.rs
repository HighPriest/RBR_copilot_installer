fn main() {
    // Compiles dialogs, string tables, manifest and version info into the exe.
    // Only meaningful on the Windows MSVC target; embed-resource no-ops elsewhere.
    embed_resource::compile("res/installer.rc", embed_resource::NONE);
    println!("cargo:rerun-if-changed=res/installer.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
}
