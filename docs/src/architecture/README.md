# Architecture

Nine diagrams of the Pacenotes system: two entity–relationship models, UML
activity diagrams for the installer and for the server, and SysML block,
internal-block and requirement diagrams.

They were drawn by reading the code, not a design document: every box,
column and status can be traced to a file named in the [code map](#code-map).
Where a rule is documented in a `CLAUDE.md`, the diagram follows it, because
a diagram that quietly disagrees with the rules is worse than no diagram.

| Diagram | Notation | Answers | Chapter |
|---|---|---|---|
| ER-1 | ER | What the server stores, and how a key binds to a package directory | [The data model](data-model.md) |
| ER-2 | ER | What one run leaves inside `<game folder>\.pacenotes` | [The data model](data-model.md) |
| A1 | UML activity | One run end to end: nine pages, three routes | [One installer run](installer.md) |
| A2 | UML activity | What scene 6 does inside the engine | [One installer run](installer.md) |
| S1 | UML activity | How one download request is answered, and where access is decided | [The server](server.md) |
| S2 | UML activity | Publishing: what `manifest gen` refuses to do | [The server](server.md) |
| SY-1 | SysML BDD | The blocks, their parts and their constraints | [The system in SysML](system.md) |
| SY-2 | SysML IBD | Deployment: parts, connections, protocols, stores | [The system in SysML](system.md) |
| SY-3 | SysML requirements | Each requirement, what satisfies it, what verifies it | [The system in SysML](system.md) |

## How they are made

Each diagram is a [D2](https://d2lang.com) source in `docs/diagrams/`,
rendered to SVG by `docs/build.sh` every time the site is built, locally and
in CI. Each source names its own layout engine in its `d2-config` vars:
dagre for the activity flows, ELK where tables and containers need it. They
render light only, because an image in a page follows the system's colour
scheme rather than the book's theme. The SVGs are never committed: what is published is
always drawn from the sources in the same commit. A source that does not
parse fails the build rather than publishing a stale picture.

Change a diagram when the structure it shows changes — a new endpoint,
module, page or stored field. Nothing regenerates the *content* by itself.

## Notation

| Element | Means |
|---|---|
| Solid arrow | control flow, or an association |
| Dashed arrow | a logical reference with no database constraint, or a compile-time dependency — the label says which |
| Filled diamond | composition: the whole owns the part |
| Diamond shape | a decision, with the guards on its outgoing edges |
| Oval | a final answer: an HTTP status, or a refusal that writes nothing |
| Filled dot | where an activity starts or ends |
| Dashed box titled "for each …" | an expansion region: the actions inside repeat |
| Colour | who acts (wizard, worker, engine, ledger, server, store); each diagram's caption names them |

## Code map

| Concern | Files |
|---|---|
| Protocol types | `shared/src/` |
| Wizard, pages, routes | `installer/src/wizard/`, `installer/res/installer.rc` |
| Tree and choice models | `installer/src/tree/`, `installer/src/choices/` |
| Worker thread and its messages | `installer/src/worker/` |
| Plan, pre-flight, execute | `installer/src/engine/` |
| Restore points | `installer/src/restore/`, `installer/src/ledger/` |
| Cache | `installer/src/cache/`, `installer/src/api/` |
| HTTP | `installer/src/http/` (WinHTTP) |
| Game detection, the running game | `installer/src/gamedir/`, `installer/src/game_process.rs` |
| Install schema | `schemas/rbr.toml`, `installer/src/schema.rs`, `installer/build/schema_table.rs` |
| The game's own step | `installer/src/install/` |
| Router and handlers | `server/src/routes.rs` |
| Access decision | `server/src/auth/` |
| Catalogue and publishing | `server/src/store/` |
| Conflict reports | `server/src/reports/` |
| Credential CLI | `server/src/admin.rs` |
| Auth schema | `server/migrations/0001_init.sql`, `server/src/db.rs` |
| Deployment | `deploy/` |

## What the diagrams deliberately leave out

- **Sequence diagrams.** The interesting ordering is inside one thread (the
  engine) or one request (the access decision); an activity diagram shows
  both without inventing lifelines.
- **A state machine.** The wizard is a property sheet with three routes, not
  a stateful protocol. The ledger's `installing → installed` is the only real
  state, and it is on ER-2.
- **A class diagram.** `shared/` is a handful of structs; ER-1 shows them
  with the keys that matter, and the [API reference](../api.md) shows the rest.
- **Topology beyond SY-2.** The Quadlet, the container and the Caddy block
  are one host; a second picture would repeat SY-2 with hostnames.
