# The data model

There are **two** entity–relationship models because the system has **two
stores that never share a database**: the server's data root and auth
database, and the installer's state inside the game folder. Nothing passes
between them but one identity and one hash.

## The identity that travels

| Concept | Where it lives | Built by |
|---|---|---|
| `game / directory / name` | server manifest, cache index, ledger, download URL | `PackageInfo::download_path()` |
| `sha256` of the zip | server manifest, the cached payload's file name, ledger entry | `manifest gen`, verified by the installer |
| case-folded path | ledger file keys only | `ledger::fold()` |

`name` is a *path* under the access directory — `codriver/polish/male/Kamil` —
and its segments **are** the installer's tree: category, then levels, then
the leaf. It is a lookup key and never joined onto a filesystem path;
`shared::valid_name` is the single check on both sides.

## ER-1: the server

![ER-1: the server's authentication, catalogue and protocol documents](diagrams/er-server.svg)

**Authentication** (libSQL, `./data/auth.db`)

| Entity | Key | Notes |
|---|---|---|
| `TOKENS` | `token_hash` | sha256 of the raw key; `expires_at` NULL means never; `revoked` 0 or 1 |
| `GRANTS` | (`token_hash`, `directory`) | one row per directory; `public` is never granted |

`GRANTS.directory` is a **logical** foreign key: it names a directory in the
catalogue, and no database constraint can check it, because the catalogue is
files on disk. That is a choice, not an omission — the server resolves the
name by manifest lookup on every request, which is also what makes a
traversal impossible.

**Catalogue** (`./data`)

| Entity | Key | Notes |
|---|---|---|
| `GAME` | `id` | a folder; `motd` and `reports` are reserved and never games |
| `ACCESS_DIRECTORY` | (`game`, `name`) | `packages/<directory>/`: the unit of access, *not* the UI category |
| `PACKAGE` | (`game`, `directory`, `name`) | version, size, sha256, updated_at, and the optional display name and exclusive group |
| `SIDECAR` | the same key | `<name>.toml` beside the zip: the **only** source of that metadata |
| `PACKAGE_REQUIRES` | (`from`, `to`) | the flattened `requires[]`, resolved within one game |
| `SCHEMA` | `game` | `<game>/schema.toml`, hashed over its **raw bytes** |
| `SCHEMA_CATEGORY` | (`game`, `id`) | `select` one or any; `install_root` a token, never a path |
| `MOTD` | `lang` | `motd/<lang>.txt`, `en` fallback, no auth |
| `CONFLICT_RECORD` | day + line | append-only JSONL: diagnostics, never input to the catalogue |
| `MANIFEST` (derived) | — | `data/manifest.json`, generated from the rows above |

The **protocol documents** are drawn too, because they are what crosses the
wire and what the installer parses: `Manifest`, `GameSchema`, `PackageInfo`,
`KeyStatus`, `ConflictReport` and `ApiError`, one module each in `shared/src/`.

## ER-2: the installer

![ER-2: what a run leaves in the game folder](diagrams/er-installer.svg)

| Entity | Key | Notes |
|---|---|---|
| `CACHE_INDEX` | — | `cache.json`; Missing, Unreadable and Loaded are three distinct states |
| `CACHE_ENTRY` | `game/directory/name` | a snapshot of `PackageInfo`, so the tree works offline |
| `CACHE_PAYLOAD` | `sha256` | `<sha256>.zip`: content addressing gives dedupe, integrity and rollback |
| `LEDGER` | — | `installed.json`, written atomically (temp file, then rename) |
| `LEDGER_ENTRY` | `game/directory/name` | `state` is `installed`, or `installing` (written ahead) |
| `LEDGER_FILE` | (`entry`, `path_folded`) | the path as written, the hash **we** wrote, and `prior` |
| `RESTORE_POINT` | UTC stamp | `restore/<stamp>/` holding `ledger.json` and `files/` |
| `SAVED_FILE` | (`point`, `path`) | the original bytes, copied only when they differ |
| `LEDGER_SNAPSHOT` | `point` | the ledger **before** the run: what lets a restore also remove later work |
| `RUN_LOG` | stamp | `logs/install-<stamp>.log`, one per run, newest five kept |

Two columns matter more than the rest:

- **`prior`** — `absent`, `identical` or `backup`. It tells a later uninstall
  whether to delete the file, leave it (it already is the original), or copy
  the saved original back. Without it, skipping byte-identical files would
  silently turn a file the user already had into one the installer owns and
  later deletes. An identical file is not even written: its hash is taken
  when it is compared.
- **`state: installing`** — written *before* extraction, so a run that dies
  halfway leaves files the next run recognises as its own, instead of taking
  them for pristine originals and backing them up.

## Where the two meet

    server PACKAGE ──download──▶ CACHE_ENTRY ──install──▶ LEDGER_ENTRY
      (game, dir, name)            (same key)               (same key)
      sha256 ──────────────────▶ CACHE_PAYLOAD ───────────▶ per-file sha256

A package is fetched once per game folder and installed from the cache,
which is why installing and restoring work with the server unreachable.
