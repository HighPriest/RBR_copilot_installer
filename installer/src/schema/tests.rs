const RBR: &str = pacenotes_shared::GAME_RBR;

/// Read at test time, so the build's table is compared with the real file, not itself.
fn schema_file(game: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("schemas")
        .join(format!("{game}.toml"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn published(sha256: &str) -> pacenotes_shared::GameSchema {
    pacenotes_shared::GameSchema {
        sha256: sha256.to_string(),
        categories: vec![pacenotes_shared::SchemaCategory {
            id: "language".into(),
            select: pacenotes_shared::SELECT_ONE.into(),
            install_root: pacenotes_shared::INSTALL_ROOT_GAME.into(),
        }],
    }
}

/// Everything else rests on the hash describing the behaviour this build has.
#[test]
fn the_compiled_in_schema_is_the_repositorys_file() {
    let on_disk = schema_file(RBR);
    let built_in = super::of(RBR).expect("this build carries an RBR schema");
    assert_eq!(built_in.toml, on_disk);
    assert_eq!(
        super::sha256_of(RBR).unwrap(),
        crate::sha256::hex_digest(&on_disk)
    );
}

/// A CRLF checkout would make every installer built there reject every server.
#[test]
fn the_schema_file_has_not_been_line_ending_converted() {
    let bytes = schema_file(RBR);
    assert!(
        !bytes.windows(2).any(|pair| pair == b"\r\n"),
        "schemas/rbr.toml has CRLF line endings: the .gitattributes `-text` rule is not in effect"
    );
}

#[test]
fn choice_categories_are_the_ones_the_schema_marks_select_one() {
    assert_eq!(
        super::choice_categories(RBR),
        vec![
            pacenotes_shared::CATEGORY_LANGUAGE,
            pacenotes_shared::CATEGORY_CODRIVER,
            pacenotes_shared::CATEGORY_STYLE,
        ]
    );
    assert!(!super::choice_categories(RBR).contains(&pacenotes_shared::CATEGORY_PACENOTES));
    assert_eq!(super::categories(RBR).len(), 4);
}

#[test]
fn every_install_root_is_one_this_build_understands() {
    assert!(super::install_roots_known(RBR));
    for category in super::categories(RBR) {
        assert_eq!(category.install_root, pacenotes_shared::INSTALL_ROOT_GAME);
    }
}

#[test]
fn a_game_this_build_does_not_know_is_reported_as_such() {
    let beamng = pacenotes_shared::GAME_BEAMNG;
    assert!(super::of(beamng).is_none());
    assert!(super::choice_categories(beamng).is_empty());
    assert_eq!(
        super::check(beamng, Some(&published("whatever"))),
        Err(super::Incompatible::GameUnknown)
    );
}

#[test]
fn a_matching_schema_allows_downloading() {
    let ours = super::sha256_of(RBR).unwrap();
    assert_eq!(super::check(RBR, Some(&published(&ours))), Ok(()));
    assert!(super::accepts(RBR, &ours));
}

#[test]
fn a_different_schema_blocks_downloading_and_names_the_unknown_category() {
    let mut theirs = published(&"0".repeat(64));
    theirs.categories.push(pacenotes_shared::SchemaCategory {
        id: "liveries".into(),
        select: pacenotes_shared::SELECT_ANY.into(),
        install_root: pacenotes_shared::INSTALL_ROOT_GAME.into(),
    });

    assert_eq!(
        super::check(RBR, Some(&theirs)),
        Err(super::Incompatible::Different {
            ours: super::sha256_of(RBR).unwrap(),
            theirs: "0".repeat(64),
            unknown_categories: vec!["liveries".to_string()],
        })
    );
}

/// Without one there is no telling whether the catalogue holds only categories this build can place.
#[test]
fn a_manifest_with_no_schema_blocks_downloading() {
    assert_eq!(
        super::check(RBR, None),
        Err(super::Incompatible::NoSchemaPublished)
    );
}

/// The categories are for the message; the hash is the check.
#[test]
fn the_hash_decides_not_the_categories() {
    let mut theirs = published(&"f".repeat(64));
    theirs.categories = super::categories(RBR)
        .iter()
        .map(|category| pacenotes_shared::SchemaCategory {
            id: category.id.into(),
            select: category.select.into(),
            install_root: category.install_root.into(),
        })
        .collect();
    assert!(super::check(RBR, Some(&theirs)).is_err());
}
