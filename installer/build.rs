#[path = "build/default_server.rs"]
mod default_server;
#[path = "build/schema_table.rs"]
mod schema_table;

fn main() {
    // Dialogs, string tables, manifest and version info; a no-op off the MSVC target.
    embed_resource::compile("res/installer.rc", embed_resource::NONE);
    println!("cargo:rerun-if-changed=res/installer.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");

    default_server::export_default_server();
    schema_table::generate_schema_table();
}
