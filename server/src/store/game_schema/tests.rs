const ONE_LANGUAGE: &[u8] = b"# a comment is part of the bytes\n[[category]]\nid = \"language\"\nselect = \"one\"\ninstall_root = \"game\"\n";

#[test]
fn the_schema_is_hashed_over_its_raw_bytes() {
    let schema = super::parse_schema(ONE_LANGUAGE).unwrap();
    assert_eq!(schema.sha256, crate::store::sha256_bytes(ONE_LANGUAGE));
    assert_eq!(schema.categories.len(), 1);
    assert_eq!(schema.categories[0].id, "language");
    assert_eq!(schema.categories[0].select, pacenotes_shared::SELECT_ONE);
    assert_eq!(
        schema.categories[0].install_root,
        pacenotes_shared::INSTALL_ROOT_GAME
    );

    let mut edited = ONE_LANGUAGE.to_vec();
    edited.extend_from_slice(b"# edited\n");
    assert_ne!(super::parse_schema(&edited).unwrap().sha256, schema.sha256);
}

#[test]
fn an_invalid_schema_is_refused() {
    let bad: [(&str, &[u8]); 6] = [
        (
            "unknown key",
            b"[[category]]\nid=\"a\"\nselect=\"one\"\ninstall_root=\"game\"\nlabel=\"A\"\n",
        ),
        (
            "bad select",
            b"[[category]]\nid=\"a\"\nselect=\"two\"\ninstall_root=\"game\"\n",
        ),
        (
            "path as root",
            b"[[category]]\nid=\"a\"\nselect=\"one\"\ninstall_root=\"C:/Windows\"\n",
        ),
        (
            "duplicate id",
            b"[[category]]\nid=\"a\"\nselect=\"one\"\ninstall_root=\"game\"\n[[category]]\nid=\"a\"\nselect=\"any\"\ninstall_root=\"game\"\n",
        ),
        (
            "bad id",
            b"[[category]]\nid=\"A b\"\nselect=\"one\"\ninstall_root=\"game\"\n",
        ),
        ("empty", b"# nothing declared\n"),
    ];
    for (case, bytes) in bad {
        assert!(super::parse_schema(bytes).is_err(), "{case} parsed");
    }
}

#[test]
fn a_schema_that_differs_from_the_bundled_one_is_flagged() {
    let schema = super::parse_schema(ONE_LANGUAGE).unwrap();
    assert_eq!(
        super::bundled_schema_warning("rbr", &schema, &[("rbr", ONE_LANGUAGE)]),
        None
    );

    let other: &[u8] = b"[[category]]\nid=\"style\"\nselect=\"one\"\ninstall_root=\"game\"\n";
    let differs = super::bundled_schema_warning("rbr", &schema, &[("rbr", other)]);
    assert!(differs.is_some_and(|warning| warning.contains("REFUSE")));

    let unbundled = super::bundled_schema_warning("rbr", &schema, &[("beamng", other)]);
    assert!(unbundled.is_some_and(|warning| warning.contains("bundles no schema for rbr")));
    assert_eq!(super::bundled_schema_warning("rbr", &schema, &[]), None);
}

#[test]
fn the_repository_schema_describes_the_real_rbr_catalogue() {
    let (game, bytes) = super::BUNDLED_SCHEMAS[0];
    assert_eq!(game, pacenotes_shared::GAME_RBR);
    let schema = super::parse_schema(bytes).unwrap();
    let ids: Vec<&str> = schema
        .categories
        .iter()
        .map(|category| category.id.as_str())
        .collect();
    assert_eq!(ids, vec!["language", "codriver", "style", "pacenotes"]);
    for category in &schema.categories {
        let expected = if pacenotes_shared::CHOICE_CATEGORIES.contains(&category.id.as_str()) {
            pacenotes_shared::SELECT_ONE
        } else {
            pacenotes_shared::SELECT_ANY
        };
        assert_eq!(category.select, expected, "{}", category.id);
        assert_eq!(category.install_root, pacenotes_shared::INSTALL_ROOT_GAME);
    }
}

/// A CR, a BOM or any non-ASCII byte from a Windows editor or checkout changes the hash.
#[test]
fn the_repository_schema_is_plain_ascii_with_lf_endings() {
    let (_, bytes) = super::BUNDLED_SCHEMAS[0];
    assert!(!bytes.contains(&b'\r'), "CRLF in schemas/rbr.toml");
    assert!(bytes.is_ascii(), "non-ASCII in schemas/rbr.toml");
}
