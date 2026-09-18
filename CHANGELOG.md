# Changelog

## 2026/09/18  

There was an error with the installer package.

```Powershell
error: current package believes it's in a workspace when it's not:
current:   E:\Git\RBR\CoPilot_InstallerV2\installer\Cargo.toml
workspace: E:\Git\RBR\CoPilot_InstallerV2\Cargo.toml

this may be fixable by adding `installer` to the `workspace.members` array of the manifest located at: E:\Git\RBR\CoPilot_InstallerV2\Cargo.toml
Alternatively, to keep it out of the workspace, add the package to the `workspace.exclude` array, or add an empty `[workspace]` table to the package's manifest.
```

Patched with addition of "workspace.exclude" in top-level `Cargo.toml`
