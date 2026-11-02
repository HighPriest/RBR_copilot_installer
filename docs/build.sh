#!/usr/bin/env bash
# Builds the documentation site into docs/book/: the architecture diagrams
# (D2 rendered to SVG), the book (mdBook), and the API reference (rustdoc)
# under book/api/. CI runs this same script (.github/workflows/docs.yml).
#
#   docs/build.sh                         the whole site
#   docs/build.sh installer-api <dir>     only the installer's API reference
#
# Needs d2 (https://d2lang.com), mdbook and cargo on PATH. The installer's
# API reference builds only on Windows; elsewhere set INSTALLER_API to a
# directory holding one (CI hands it over from its Windows job), or the site
# is built without it — unless REQUIRE_INSTALLER_API=1, which fails instead.
set -euo pipefail

docs="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(dirname "$docs")"

need() {
    command -v "$1" >/dev/null || {
        echo "docs/build.sh: $1 is not on PATH — $2" >&2
        exit 1
    }
}

# The installer's library and its Windows binary share a crate name, so each
# gets a rustdoc run and a target directory of its own. build.rs reads the
# default server from the file PACENOTES_ENV_FILE names: the example one,
# because rustdoc prints that address and a published page must never carry
# a deployment's own. RUSTFLAGS would replace the crate's own flags, which
# its build.rs refuses.
installer_api() {
    local out="$1"
    mkdir -p "$out"
    (
        cd "$root/installer"
        unset RUSTFLAGS
        export PACENOTES_ENV_FILE=.env.example
        export RUSTDOCFLAGS="-D warnings"
        for part in lib bin; do
            local what=(--lib)
            [ "$part" = bin ] && what=(--bin pacenotes-installer)
            CARGO_TARGET_DIR="$root/target/docs-installer-$part" \
                cargo doc --locked --no-deps --document-private-items "${what[@]}"
            rm -rf "${out:?}/$part"
            cp -r "$root/target/docs-installer-$part/x86_64-pc-windows-msvc/doc" "$out/$part"
        done
    )
}

if [ "${1:-}" = installer-api ]; then
    installer_api "${2:?usage: docs/build.sh installer-api <dir>}"
    exit 0
fi

need d2 "install it from https://d2lang.com"
need mdbook "install it from https://github.com/rust-lang/mdBook/releases"

echo "== diagrams"
diagrams="$docs/src/architecture/diagrams"
mkdir -p "$diagrams"
for source in "$docs"/diagrams/*.d2; do
    name="$(basename "$source" .d2)"
    # Each source names its layout engine, theme and padding in its own
    # d2-config vars. Light only: an <img> follows the system's colour scheme,
    # not the book's theme, and a dark variant broke the coloured boxes.
    d2 "$source" "$diagrams/$name.svg"
done

echo "== book"
# mdBook empties build-dir first, so the API reference goes in after it.
mdbook build "$docs"

echo "== API reference"
api="$docs/book/api"
mkdir -p "$api"
(
    cd "$root"
    RUSTDOCFLAGS="-D warnings" CARGO_TARGET_DIR="$root/target/docs-workspace" \
        cargo doc --locked --workspace --no-deps --document-private-items
)
cp -r "$root/target/docs-workspace/doc/." "$api/"

if [ -n "${INSTALLER_API:-}" ]; then
    cp -r "$INSTALLER_API" "$api/installer"
else
    case "$(uname -s)" in
        MINGW* | MSYS* | CYGWIN*) installer_api "$api/installer" ;;
        *)
            if [ "${REQUIRE_INSTALLER_API:-0}" = 1 ]; then
                echo "docs/build.sh: the installer's API reference is required, and INSTALLER_API is not set" >&2
                exit 1
            fi
            echo "docs/build.sh: no installer API reference here (Windows only); its links will be dead" >&2
            ;;
    esac
fi

echo "== done: $docs/book/index.html"
