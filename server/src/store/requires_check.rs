/// Every `requires` entry that is malformed, dangling, crosses into another
/// game, or leads a public package to a protected one.
pub fn requires_problems(manifest: &pacenotes_shared::Manifest) -> Vec<String> {
    let mut problems = Vec::new();
    for package in &manifest.packages {
        let owner = format!("{}/{}", package.directory, package.name);
        for text in &package.requires {
            let Some(reference) = pacenotes_shared::PackageRef::parse(text) else {
                problems.push(format!(
                    "{owner} requires {text:?}, which is not a \"directory/name\" reference"
                ));
                continue;
            };
            let Some(dependency) = manifest.get(&reference) else {
                problems.push(format!(
                    "{owner} requires {reference}, which does not exist"
                ));
                continue;
            };
            if package.is_public() && !dependency.is_public() {
                problems.push(format!(
                    "{owner} is public but requires {reference}, which is not — \
                     anonymous users would see a package they cannot complete"
                ));
            }
            if dependency.game != package.game {
                problems.push(format!(
                    "{owner} (game {}) requires {reference} (game {})",
                    package.game, dependency.game
                ));
            }
        }
    }
    problems
}
