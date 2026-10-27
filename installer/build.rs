#[path = "build/default_server.rs"]
mod default_server;
#[path = "build/schema_table.rs"]
mod schema_table;

fn main() {
    // Dialogs, string tables, manifest and version info; a no-op off the MSVC target.
    embed_resource::compile("res/installer.rc", embed_resource::NONE);
    println!("cargo:rerun-if-changed=res/installer.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    // The .rc only NAMES the icon, so without this a new icon is not a
    // reason to rebuild and the old one stays in the binary — which cost
    // an afternoon once, measuring a size change that had not happened.
    println!("cargo:rerun-if-changed=../rbr.ico");

    default_server::export_default_server();
    schema_table::generate_schema_table();
}
