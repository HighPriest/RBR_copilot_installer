#[path = "build/default_server.rs"]
mod default_server;
#[path = "build/release_version.rs"]
mod release_version;
#[path = "build/schema_table.rs"]
mod schema_table;

fn main() {
    // First, since the .rc includes the version header it writes.
    release_version::export_release_version();
    // Dialogs, string tables, manifest and version info; a no-op off the MSVC target.
    embed_resource::compile("res/installer.rc", embed_resource::NONE);
    println!("cargo:rerun-if-changed=res/installer.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    // The .rc only names the icon, so without this a new icon leaves the old one in the binary.
    println!("cargo:rerun-if-changed=../rbr.ico");

    default_server::export_default_server();
    schema_table::generate_schema_table();
}
