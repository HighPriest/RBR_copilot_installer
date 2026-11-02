# The system in SysML

## Blocks

![SY-1: the system and its blocks](diagrams/sysml-bdd.svg)

| Block | Responsibility | Constraint |
|---|---|---|
| **Installer** | the Windows GUI: nine pages, one worker thread, the install engine | `.exe` under 700 kB; no tokio, reqwest, rustls, hyper or GUI crate; only DLLs Windows ships |
| **Package Server** | catalogue, auth, downloads, conflict reports, the admin CLI | one static binary; TLS is the reverse proxy's job |
| **Shared Protocol** | the wire types both sides compile against | serde only: no I/O, no async, no HTTP |
| **Package Catalogue** | zips, sidecars, schemas, the generated manifest, the MOTD, reports | one folder per game under `--data` |
| **Auth Database** | keys and their grants | libSQL; raw keys are never stored |
| **Game Installation** | the game folder a run targets | the only place anything is written |

The composition is deliberate: `shared/` is the *only* code the two binaries
have in common, and it holds no behaviour that could grow a runtime the
installer cannot afford.

## Deployment and interfaces

![SY-2: deployment and interfaces](diagrams/sysml-ibd.svg)

| From | To | Carries |
|---|---|---|
| Player | Installer | choices; the one thing it sends anywhere is an anonymous conflict report |
| Installer | Caddy | HTTPS: `GET /packages`, `/download/...`, `/motd`, `/whoami`; `POST /report/conflict` |
| Caddy | Server | plain HTTP on `:8080`; TLS ends at the proxy |
| Server | Catalogue | `manifest.json` and the zip streams |
| Server | Auth DB | `SELECT ... FROM tokens LEFT JOIN grants` |
| Installer | Game Installation | the extracted overlay, and the cache, ledger, restore points and logs in `.pacenotes` |
| Operator | Catalogue, Auth DB | copy a zip and sidecar, `manifest gen`; `token mint`, `grant`, `revoke` |

The installer's HTTPS is WinHTTP, so the TLS handshake is Schannel's: that
is why the binary ships no TLS crate, and why the server can speak plain
HTTP behind the proxy. Redirects are followed by the installer itself, never
by WinHTTP, so the access key never leaves the origin it was meant for.

## Requirements

![SY-3: requirements, what satisfies them, what verifies them](diagrams/sysml-requirements.svg)

| Id | Requirement | Satisfied by | Verified by |
|---|---|---|---|
| R-SIZE | the `.exe` stays under 700 kB and imports only DLLs Windows ships | Installer | `installer/tests/Test-BinarySize.ps1` |
| R-TLS | HTTPS through WinHTTP and Schannel; no TLS, async or GUI crate | Installer / http | `installer/tests/Test-PublicEndpoint.ps1` |
| R-REDIRECT | redirects followed by the installer; the key never leaves its origin | Installer / http | `installer/tests/Test-Redirects.ps1` |
| R-OVERLAY | a package is a game-root overlay; its content is never interpreted | Installer / engine | `installer/tests/applying_a_selection.rs` |
| R-LOCAL | nothing is written outside the game folder | Installer / cache + ledger | `installer/src/cache/tests.rs`, `installer/src/ledger/tests.rs` |
| R-OFFLINE | install and restore work from the cache | Installer / engine + restore | `installer/tests/restoring_a_game_folder.rs` |
| R-SCHEMA | an unknown schema hash blocks downloading for that game only | Installer / schema | `installer/src/schema/tests.rs` |
| R-AUTH | bearer keys stored as sha256, grants per directory, no tiers | Server / auth | the `decide` table in `server/src/auth/access_decision.rs` |
| R-ENUM | statuses never enumerate; there is no 403 | Server / routes | `server/tests/access_matrix.rs` |
| R-PATH | URL parts are lookup keys, never paths | Server / store | `server/tests/package_download.rs` |
| R-PURE | the manifest is a pure function of zips, sidecars and schemas | Server / store | `server/tests/publishing.rs` |
| R-REPORT | reports are unauthenticated, capped, validated and rate limited | Server / reports | `server/tests/conflict_reports.rs` |
