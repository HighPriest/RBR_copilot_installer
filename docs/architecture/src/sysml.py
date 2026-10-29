"""SysML diagrams: block definition, internal block (deployment) and requirements.

Run from the repository root:
    python docs/architecture/src/sysml.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from diagramkit import Ink, Kit  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def block(c, x, y, w, name, stereotype, sections, accent=Ink.server, fill="#FFFFFF"):
    header = 56
    heights = [26 + 20 * len(lines) + 10 for _, lines in sections]
    h = header + sum(heights)
    c.rrect(x, y, w, h, radius=8, fill=fill, outline=accent, width=1.7)
    c.d.rectangle([x + 1, y + 1, x + w - 1, y + header], fill=accent)
    c.text(x + 12, y + 8, stereotype, c.reg(11), "#FFFFFF")
    c.text(x + 12, y + 26, name, c.semi(16), "#FFFFFF")
    yy = y + header
    for (title, lines), sh in zip(sections, heights):
        c.d.line([(x + 1, yy), (x + w - 1, yy)], fill=accent, width=1)
        c.text(x + 12, yy + 5, title, c.bold(9.5), accent)
        ly = yy + 22
        for line in lines:
            c.text(x + 14, ly, line, c.mono(11), Ink.ink)
            ly += 20
        yy += sh
    c.boxes[name] = (x, y, w, h)
    return (x, y, w, h)


def diamond(c, x, y, color, size=9):
    c.poly([(x, y - size), (x + size * 1.3, y), (x, y + size), (x - size * 1.3, y)],
           fill=color, outline=color, width=1)


def bdd(path):
    c = Kit(1900, 1210)
    c.title(40, 22, "SY-1 — SysML Block Definition Diagram: the system and its blocks",
            "What the system is made of, and what each part is responsible for. Composition (filled "
            "diamond) means 'is part of'; the dashed arrows are compile-time dependencies on the shared "
            "protocol crate.")

    system = block(c, 80, 110, 1720, "Pacenotes System", "«block»", [
        ("parts", ["installer : Installer  [1]        server : Package Server  [1]        "
                   "protocol : Shared Protocol  [1 library]"]),
    ], accent=Ink.ink)

    installer = block(c, 80, 380, 520, "Installer", "«block»  Windows GUI", [
        ("parts", ["wizard : PropertySheet  [9 pages]",
                   "worker : Thread  [1]",
                   "engine : Install Engine",
                   "cache, ledger, restore, runlog",
                   "http : WinHTTP client"]),
        ("constraints / values", ["exe <= 600 kB (release gate)",
                                  "no tokio, reqwest, rustls, GUI crate"]),
    ], accent=Ink.client)

    protocol = block(c, 660, 380, 520, "Shared Protocol", "«block»  shared/ crate", [
        ("types", ["Manifest { packages, schemas }",
                   "PackageInfo, PackageRef",
                   "GameSchema, SchemaCategory",
                   "KeyStatus, ConflictReport, ApiError"]),
        ("constraint", ["serde only - no I/O, no async, no HTTP"]),
    ], accent=Ink.protocol)

    server = block(c, 1240, 380, 560, "Package Server", "«block»  single static binary", [
        ("parts", ["routes : axum Router",
                   "auth : decide(token, dir, now)",
                   "store : ManifestStore + manifest gen",
                   "reports : validate + rate limit",
                   "admin : token CLI"]),
        ("values", ["libSQL auth DB (file or Turso)",
                    "plain HTTP - Caddy terminates TLS"]),
    ], accent=Ink.server)

    game = block(c, 80, 820, 520, "Game Installation", "«block»  external", [
        ("values", ["<game folder> - the only place anything is written",
                    "detected by its marker .exe",
                    "Plugins/Pacenote/... for RBR"]),
    ], accent=Ink.disk, fill="#FAFBFD")

    catalogue = block(c, 660, 820, 520, "Package Catalogue", "«block»  data root", [
        ("values", ["<game>/packages/<directory>/<name>.zip",
                    "<name>.toml sidecar",
                    "<game>/schema.toml (hashed)",
                    "manifest.json (generated)",
                    "motd/<lang>.txt, reports/*.jsonl"]),
    ], accent=Ink.server, fill="#FAFBFD")

    authdb = block(c, 1240, 820, 330, "Auth Database", "«block»  libSQL", [
        ("tables", ["tokens(token_hash PK, ...)",
                    "grants(token_hash, directory)"]),
    ], accent=Ink.auth, fill="#FAFBFD")

    # composition: every part hangs from the system block
    for child, frac in ((installer, 0.5), (protocol, 0.5), (server, 0.5)):
        cx = child[0] + child[2] * frac
        c.arrow([(cx, system[1] + system[3]), (cx, child[1])], color=Ink.ink, width=1.5, head=False)
        diamond(c, cx, system[1] + system[3], Ink.ink)
        c.tag(cx + 12, (system[1] + system[3] + child[1]) / 2, "1", c.bold(11), Ink.ink, align="left")

    # associations
    c.arrow([(installer[0] + installer[2] / 2, installer[1] + installer[3]),
             (installer[0] + installer[2] / 2, game[1])], color=Ink.line,
            label="installs into", label_font=c.reg(12))
    c.arrow([(1270, server[1] + server[3]), (1270, 700), (950, 700), (950, catalogue[1])],
            color=Ink.line, label="serves from", label_font=c.reg(12), label_at=0.55)
    c.arrow([(1450, server[1] + server[3]), (1450, authdb[1])], color=Ink.line,
            label="reads", label_font=c.reg(12), label_offset=(36, 0))

    # compile-time dependency on the protocol crate
    c.arrow([(installer[0] + installer[2], 470), (protocol[0], 470)], color=Ink.protocol,
            dash=6, label="«use»", label_font=c.reg(11))
    c.arrow([(protocol[0] + protocol[2], 470), (server[0], 470)], color=Ink.protocol,
            dash=6, label="«use»", label_font=c.reg(11))

    c.note(80, 1040, 620,
           "Two deliverables, one repository: the Windows .exe and the server binary. 'shared/' is the "
           "only code they have in common, and it holds no I/O - which is what stops the installer "
           "growing a runtime it cannot afford.", font_size=12)
    c.legend(900, 1050, [(Ink.client, "installer"), (Ink.server, "server"),
                         (Ink.protocol, "shared protocol"), (Ink.auth, "auth store")], columns=4)
    return c.save(os.path.join(path, "sysml-bdd.png"))


def port(c, x, y, label, side, accent):
    s = 8
    c.d.rectangle([x - s, y - s, x + s, y + s], fill="#FFFFFF", outline=accent, width=2)
    if not label:
        return
    if side == "bottom":
        c.text(x + 12, y + 9, label, c.reg(10.5), accent)
    else:
        c.text(x, y - 21, label, c.reg(10.5), accent, anchor="ma")


def ibd(path):
    c = Kit(1900, 1210)
    c.title(40, 22, "SY-2 — SysML Internal Block Diagram: deployment and interfaces",
            "The parts as they are actually wired: ports on the edges, the protocol each connection "
            "carries, and where the data lives. The installer talks HTTPS to a reverse proxy; the "
            "server itself speaks plain HTTP, which is why its binary ships no TLS crate.")

    player = block(c, 60, 300, 190, "Player", "«actor»", [("", ["runs the wizard"])], accent=Ink.disk)
    inst = block(c, 320, 230, 440, "Installer : Pacenotes System", "«part»  Windows .exe", [
        ("provides", ["Pages, tree, choices, progress"]),
        ("requires", ["WinHTTP, Schannel (OS)"]),
    ], accent=Ink.client)
    caddy = block(c, 850, 300, 200, "Caddy", "«part»  reverse proxy", [
        ("", ["TLS termination", "HTTP/1.1 and 2"]),
    ], accent=Ink.muted, fill="#FAFBFD")
    server = block(c, 1150, 240, 430, "Server : Package Server", "«part»  container", [
        ("provides", ["/health /whoami /packages", "/download /motd /report/conflict"]),
        ("requires", ["data root, auth DB"]),
    ], accent=Ink.server)
    game = block(c, 320, 640, 440, "Game Installation", "«part»  file system", [
        ("", ["<game folder>\\.pacenotes\\",
              "overlay files in their final place"]),
    ], accent=Ink.disk, fill="#FAFBFD")
    cat = block(c, 850, 640, 380, "Package Catalogue", "«part»  file system", [
        ("", ["manifest.json", "<game>/packages/**.zip"]),
    ], accent=Ink.server, fill="#FAFBFD")
    db = block(c, 1290, 640, 310, "Auth DB", "«part»  libSQL", [("", ["tokens, grants"])],
               accent=Ink.auth, fill="#FAFBFD")
    op = block(c, 950, 900, 230, "Operator", "«actor»", [("", ["publishes zips,", "mints keys"])],
               accent=Ink.disk)

    # ports
    port(c, inst[0] + inst[2], 360, "", "", Ink.client)
    port(c, caddy[0], 360, "", "", Ink.muted)
    port(c, caddy[0] + caddy[2], 360, "", "", Ink.muted)
    port(c, server[0], 360, "", "", Ink.server)
    port(c, inst[0] + inst[2] / 2, inst[1] + inst[3], "pacenotes", "bottom", Ink.client)
    port(c, 1250, server[1] + server[3], "files", "bottom", Ink.server)
    port(c, 1500, server[1] + server[3], "SQL", "bottom", Ink.server)
    port(c, player[0] + player[2], player[1] + player[3] / 2, "", "right", Ink.disk)

    # connectors
    c.arrow([(player[0] + player[2], player[1] + player[3] / 2),
             (inst[0], player[1] + player[3] / 2)], color=Ink.disk, label="runs",
            label_font=c.reg(11))
    c.arrow([(inst[0] + inst[2], 360), (caddy[0], 360)], color=Ink.client)
    c.arrow([(caddy[0] + caddy[2], 360), (server[0], 360)], color=Ink.server)
    c.text(805, 372, "HTTPS", c.reg(11), Ink.client, anchor="ma")
    c.text(1100, 372, "HTTP", c.reg(11), Ink.server, anchor="ma")
    c.text(1060, 180, "GET /packages · /download · /motd · /whoami · POST /report/conflict",
           c.reg(12), Ink.muted, anchor="ma")

    c.arrow([(inst[0] + inst[2] / 2, inst[1] + inst[3]), (inst[0] + inst[2] / 2, game[1])],
            color=Ink.client, label="extract the overlay; write cache, ledger, restore points, logs",
            label_font=c.reg(11), label_offset=(-30, 0), label_at=0.5)
    c.arrow([(1250, server[1] + server[3]), (1250, 600), (1040, 600), (1040, cat[1])],
            color=Ink.server, label="read manifest.json, stream the zip", label_font=c.reg(11),
            label_at=0.55, label_offset=(0, -12))
    c.arrow([(1500, server[1] + server[3]), (1500, 600), (1445, 600), (1445, db[1])],
            color=Ink.auth, label="SELECT tokens LEFT JOIN grants", label_font=c.reg(11),
            label_at=0.55, label_offset=(0, -12))

    # administration, out of band
    c.arrow([(op[0] + op[2] / 2, op[1]), (op[0] + op[2] / 2, 870), (1040, 870), (1040, cat[1] + cat[3])],
            color=Ink.warn, dash=6, label="copy zip + sidecar, run manifest gen",
            label_font=c.reg(11), label_at=0.45, label_offset=(0, -12))
    c.arrow([(op[0] + op[2], op[1] + 30), (1250, op[1] + 30), (1250, 720), (db[0], 720)],
            color=Ink.warn, dash=6, label="token mint / grant / revoke (CLI)",
            label_font=c.reg(11), label_at=0.55, label_offset=(0, -12))

    c.note(60, 1075, 700,
           "One deployment, one data root: the same server process serves every game, and the first "
           "folder level under --data names the game. A second game is a folder, not a second server.",
           font_size=12)
    c.note(820, 1075, 1000,
           "The auth DB is a file the admin CLI and the server both write, which is why a local SQLite "
           "is opened with busy_timeout and WAL. Switching to Turso means changing --db-url and nothing "
           "else.", font_size=12)
    return c.save(os.path.join(path, "sysml-ibd.png"))


def requirements(path):
    c = Kit(1900, 1420)
    c.title(40, 22, "SY-3 — SysML Requirement Diagram: what the system must do, and what proves it",
            "Each requirement is traced to the block that satisfies it and to the test artefact that "
            "verifies it. The ids are the section names used in the repository's own rule documents.")

    reqs = [
        ("R-SIZE", "The installer .exe stays under 600 kB.", "Installer",
         "tests/Test-BinarySize.ps1"),
        ("R-TLS", "HTTPS goes through WinHTTP/Schannel: no TLS, async or GUI crate in the installer.",
         "Installer / http", "tests/Test-PublicEndpoint.ps1"),
        ("R-OVERLAY", "A package is a game-root overlay. The installer never interprets package content.",
         "Installer / engine", "tests/engine_execute.rs"),
        ("R-LOCAL", "Nothing is written outside the game folder; deleting it removes every trace.",
         "Installer / cache + ledger", "tests/cache.rs, ledger.rs"),
        ("R-OFFLINE", "Installing and restoring work from the cache with no server.",
         "Installer / engine + restore", "tests/restore_execute.rs"),
        ("R-SCHEMA", "An unknown install-schema hash blocks downloading for that game only.",
         "Installer / schema", "tests/schema.rs"),
        ("R-AUTH", "Bearer tokens; only sha256 is stored; grants name directories; no tiers.",
         "Server / auth", "server: auth::decide table"),
        ("R-ENUM", "Statuses never enumerate: no 403, 401 vs 404 by credential, not by existence.",
         "Server / routes", "server/tests/api.rs matrix"),
        ("R-PATH", "URL parts are manifest lookup keys, never joined into a filesystem path.",
         "Server / store", "api.rs traversal tests"),
        ("R-PURE", "manifest.json is a pure function of zips, sidecars and schemas.",
         "Server / manifest gen", "store.rs unit tests"),
        ("R-REPORT", "Conflict reports are unauthenticated, capped, validated and rate limited.",
         "Server / reports", "reports unit + api tests"),
    ]

    top = 140
    row = 96
    for i, (rid, text, block_name, test) in enumerate(reqs):
        y = top + i * row
        c.rrect(60, y, 700, 78, radius=6, fill="#FFFDF5", outline=Ink.decision_edge, width=1.3)
        c.text(74, y + 8, "«requirement»", c.reg(10), Ink.warn)
        c.text(74, y + 24, rid, c.bold(13), Ink.ink)
        c.paragraph(160, y + 22, text, c.reg(12), 580, Ink.ink, 1.2)

        c.arrow([(760, y + 39), (930, y + 39)], color=Ink.ok, width=1.4,
                label="«satisfy»", label_font=c.reg(10), label_at=0.5, label_offset=(0, -10))
        c.rrect(940, y + 16, 380, 46, radius=6, fill="#EEF7F2", outline=Ink.ok, width=1.3)
        c.text(956, y + 24, "«block»", c.reg(9.5), Ink.ok)
        c.text(956, y + 38, block_name, c.mono(11.5), Ink.ink)

        c.arrow([(1320, y + 39), (1430, y + 39)], color=Ink.server, width=1.4,
                label="«verify»", label_font=c.reg(10), label_at=0.5, label_offset=(0, -10))
        c.rrect(1440, y + 16, 420, 46, radius=6, fill="#F3F7FC", outline=Ink.server, width=1.3)
        c.text(1454, y + 24, "«test case»", c.reg(9.5), Ink.server)
        c.text(1454, y + 38, test, c.mono(11), Ink.ink)

    c.note(60, 1220, 700,
           "Some rules cannot be tested by a program, only reviewed: 'the installer never interprets "
           "package content' is a structural claim about the code, and 'the operator is the only one "
           "who can fix a catalogue' is a process claim. They are listed here because a requirement "
           "nobody traces is a requirement nobody keeps.", font_size=12)
    c.note(940, 1220, 920,
           "The size gate and the schema hash are the only two HARD numeric gates in the system. "
           "Everything else is an invariant enforced by construction: a manifest lookup cannot produce "
           "a path outside the data root, and an unauthenticated endpoint cannot be made to write "
           "outside its reports directory.", font_size=12)
    return c.save(os.path.join(path, "sysml-requirements.png"))


if __name__ == "__main__":
    print(bdd(OUT))
    print(ibd(OUT))
    print(requirements(OUT))