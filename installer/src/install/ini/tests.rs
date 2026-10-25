#[test]
fn the_existing_spacing_around_the_equals_is_kept() {
    assert_eq!(
        super::set_ini_key("[S]\na = b\n", "S", "a", "c"),
        "[S]\na = c\n"
    );
    assert_eq!(
        super::set_ini_key("[S]\na=b\n", "S", "a", "c"),
        "[S]\na=c\n"
    );
}

#[test]
fn an_inline_comment_survives_a_value_change() {
    assert_eq!(
        super::set_ini_key(
            "[S]\nlanguage=polish  ; I prefer this\n",
            "S",
            "language",
            "english"
        ),
        "[S]\nlanguage=english  ; I prefer this\n"
    );
}

#[test]
fn a_semicolon_inside_a_value_is_not_a_comment() {
    assert_eq!(
        super::set_ini_key("[S]\nlist=a;b;c\n", "S", "list", "x;y"),
        "[S]\nlist=x;y\n"
    );
}

#[test]
fn a_commented_out_key_is_never_the_key() {
    assert_eq!(
        super::set_ini_key(
            "[S]\n;language=english\nlanguage=polish\n",
            "S",
            "language",
            "czech"
        ),
        "[S]\n;language=english\nlanguage=czech\n"
    );
}

#[test]
fn a_key_is_set_only_in_its_own_section() {
    let text = "[OTHER]\r\nlanguage=french\r\n[SETTINGS]\r\nlanguage=polish\r\n";
    assert_eq!(
        super::set_ini_key(text, "SETTINGS", "language", "english"),
        "[OTHER]\r\nlanguage=french\r\n[SETTINGS]\r\nlanguage=english\r\n"
    );
}

/// At the end of the file it would belong to whichever section came last.
#[test]
fn a_missing_key_is_added_to_its_own_section() {
    let text = "[SETTINGS]\r\nenableGUI=1\r\n[OTHER]\r\nx=1\r\n";
    assert_eq!(
        super::set_ini_key(text, "SETTINGS", "language", "english"),
        "[SETTINGS]\r\nenableGUI=1\r\nlanguage=english\r\n[OTHER]\r\nx=1\r\n"
    );
}

#[test]
fn a_missing_section_is_appended() {
    assert_eq!(
        super::set_ini_key("[OTHER]\nx=1\n", "SETTINGS", "language", "english"),
        "[OTHER]\nx=1\n\n[SETTINGS]\nlanguage=english\n"
    );
}

#[test]
fn the_files_line_endings_are_preserved() {
    assert_eq!(
        super::set_ini_key("[S]\r\na=b\r\n", "S", "a", "c"),
        "[S]\r\na=c\r\n"
    );
    assert_eq!(
        super::set_ini_key("[S]\na=b\n", "S", "a", "c"),
        "[S]\na=c\n"
    );
}

#[test]
fn a_file_without_a_final_newline_keeps_its_shape() {
    assert_eq!(super::set_ini_key("[S]\na=b", "S", "a", "c"), "[S]\na=c");
}
