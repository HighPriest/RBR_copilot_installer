//! A category is the first segment of `PackageInfo::name`: the folder a package
//! sits in under its access directory. The set is open; these are the known ones.

pub const CATEGORY_LANGUAGE: &str = "language";
pub const CATEGORY_CODRIVER: &str = "codriver";
pub const CATEGORY_STYLE: &str = "style";
pub const CATEGORY_PACENOTES: &str = "pacenotes";

/// Lets the server spot a category name filed where an access directory belongs.
pub const KNOWN_CATEGORIES: [&str; 4] = [
    CATEGORY_LANGUAGE,
    CATEGORY_CODRIVER,
    CATEGORY_STYLE,
    CATEGORY_PACENOTES,
];

/// The categories the Configuration scene asks for exactly one pick from, in display order.
pub const CHOICE_CATEGORIES: [&str; 3] = [CATEGORY_LANGUAGE, CATEGORY_CODRIVER, CATEGORY_STYLE];
