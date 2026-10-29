"""UML activity diagrams for the server.

S1: the request pipeline, with the access decision that guards a download.
S2: publishing - what 'manifest gen' does before manifest.json is written.

Run from the repository root:
    python docs/architecture/src/server_activity.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from diagramkit import Ink, Kit  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

WHO = {
    "HANDLER": ("#EAF1FA", "#7FA0C6"),
    "AUTH": ("#F1EAF9", "#9A7FC0"),
    "STORE": ("#E7F4EE", "#5FA183"),
    "CLI": ("#FDF2E7", "#C99A5B"),
    "FS": ("#EEF1F5", "#8A97A8"),
    "REPORT": ("#F7EDF1", "#B07C90"),
}


def box(c, cx, y, w, text, who, tag=None):
    fill, edge = WHO[who]
    return c.action(cx, y, w, text, tag=tag if tag is not None else who, accent=edge, fill=fill)


def pill(c, cx, y, w, text, tone=Ink.muted, fill="#FFFFFF"):
    h = 38
    c.rrect(cx - w / 2, y, w, h, radius=h / 2, fill=fill, outline=tone, width=1.5)
    c.text(cx, y + h / 2 - 1, text, c.reg(12), tone, anchor="mm")
    return (cx - w / 2, y, w, h)


def server_request(path):
    c = Kit(1800, 1400)
    c.title(40, 22, "S1 — Server: how one request is answered",
            "Partitions are the modules the code is split into: routes.rs parses and answers, auth.rs "
            "decides, store.rs turns keys into a path on disk. The flow shown is GET /download, the "
            "one that touches both the database and the filesystem.")

    lanes = [("CLIENT", 60, 260), ("routes.rs", 340, 440), ("auth.rs", 800, 400), ("store.rs", 1220, 480)]
    top = 110
    for name, x, w in lanes:
        c.rrect(x, top, w, 1140, radius=12, fill="#FAFBFD", outline=Ink.rule, width=1.2)
        c.text(x + 16, top + 12, name, c.semi(15), Ink.muted)
        c.dashed([(x + w, top + 42), (x + w, top + 1246)], Ink.rule, 1.2, 8, 7)

    c.text(588, 168, "GET /download/…", c.mono(12), Ink.muted)
    c.start(560, 178)
    d1 = c.decision(560, 200, 330, "valid ids?")
    p404a = pill(c, 190, 226, 250, "404 · nothing could exist", Ink.server)
    d2 = c.decision(560, 320, 330, "public directory?")
    a1 = box(c, 1000, 400, 330, "bearer header -> well_formed()? garbage costs no query", "AUTH")
    a2 = box(c, 1000, 480, 330, "load_token(): tokens LEFT JOIN grants, decoded fail-closed", "AUTH")
    d3 = c.decision(1000, 570, 340, "decide(): allow, unauthorized, hidden")
    p401 = pill(c, 190, 586, 210, "401 · no live key", Ink.auth)
    p404b = pill(c, 190, 681, 300, "404 · no grant, or no such package", Ink.server)
    s1 = box(c, 1450, 700, 420, "manifest lookup: only a hit yields a path on disk", "STORE")
    d4 = c.decision(1450, 790, 340, "hit?")
    s2 = box(c, 1450, 880, 420, "stat the file: size == manifest.size?", "STORE")
    p503 = pill(c, 190, 931, 270, "503 · being copied, or stale manifest", Ink.warn)
    d5 = c.decision(560, 980, 330, "If-None-Match matches sha256?")
    p304 = pill(c, 190, 996, 210, "304 · not modified", Ink.muted)
    h3 = box(c, 560, 1100, 400, "stream it: 200, ETag, Content-Length, Content-Disposition", "HANDLER")
    p200 = pill(c, 190, 1166, 230, "200 · the zip streams", Ink.ok)

    c.arrow([(560, 183), (560, d1[1])])
    c.arrow([(d1[0], d1[1] + d1[3] / 2), (p404a[0] + p404a[2], d1[1] + d1[3] / 2)],
            label="no", label_at=0.5, label_offset=(0, -12))
    c.arrow([(560, d1[1] + d1[3]), (560, d2[1])])
    c.arrow([(d2[0] + d2[2], d2[1] + d2[3] / 2), (1450, d2[1] + d2[3] / 2), (1450, s1[1])],
            label="yes: no credential is needed", label_at=0.5, label_offset=(0, -12))
    c.arrow([(560, d2[1] + d2[3]), (560, 420), (a1[0], 420)])
    c.arrow([(1000, a1[1] + a1[3]), (1000, a2[1])])
    c.arrow([(1000, a2[1] + a2[3]), (1000, d3[1])])
    c.arrow([(d3[0], d3[1] + d3[3] / 2), (p401[0] + p401[2], d3[1] + d3[3] / 2)],
            label="unauthorized", label_at=0.62, label_offset=(0, -12))
    c.arrow([(1000, d3[1] + d3[3]), (1000, 700), (p404b[0] + p404b[2], 700)],
            label="hidden", label_at=0.14, label_offset=(0, -12))
    c.arrow([(d3[0] + d3[2], d3[1] + d3[3] / 2), (1215, d3[1] + d3[3] / 2), (1215, 730), (s1[0], 730)],
            label="allow", label_at=0.62, label_offset=(0, -12))
    c.arrow([(1450, s1[1] + s1[3]), (1450, d4[1])])
    c.arrow([(d4[0], d4[1] + d4[3] / 2), (340, d4[1] + d4[3] / 2), (340, p404b[1] + p404b[3])],
            label="no", label_at=0.12, label_offset=(0, -12))
    c.arrow([(1450, d4[1] + d4[3]), (1450, s2[1])])
    c.arrow([(s2[0], s2[1] + s2[3] / 2), (1180, s2[1] + s2[3] / 2), (1180, 950),
             (p503[0] + p503[2], 950)], label="no", label_at=0.2, label_offset=(0, -12))
    c.arrow([(1450, s2[1] + s2[3]), (1450, 958), (560, 958), (560, d5[1])])
    c.arrow([(d5[0], d5[1] + d5[3] / 2), (p304[0] + p304[2], d5[1] + d5[3] / 2)],
            label="yes", label_at=0.5, label_offset=(0, -12))
    c.arrow([(560, d5[1] + d5[3]), (560, h3[1])], label="no", label_offset=(18, 0))
    c.arrow([(h3[0], h3[1] + 22), (350, h3[1] + 22), (350, 1185), (p200[0] + p200[2], 1185)])

    c.note(60, 1270, 540,
           "The statuses are a design decision, not an accident: 401 answers 'no live key' whether or "
           "not the package exists; 404 answers 'no grant' and 'no such package' identically; there is "
           "no 403, because 'exists but not for you' is the enumeration signal this removes.",
           font_size=11)
    c.note(640, 1270, 540,
           "Every response leaves one INFO line on target 'access': method, path+query, status, bytes, "
           "milliseconds to the response head, the first 12 hex characters of the key's HASH, and the "
           "user agent. The raw key is never logged.", font_size=11)
    c.note(1220, 1270, 520,
           "/packages is the exception to the statuses above: it never 4xxs. It silently filters to "
           "public + granted directories, and falls back to the public listing when the database is "
           "down, so a blip cannot take the free catalogue with it.", font_size=11)
    return c.save(os.path.join(path, "activity-server.png"))


def manifest_gen(path):
    c = Kit(1700, 1480)
    c.title(40, 22, "S2 — Server: publishing (manifest gen)",
            "Publishing is copying a zip into place and running one command. The command is STRICT: a "
            "game without an install schema publishes nothing, a sidecar with an unknown key withholds "
            "its package, and the run refuses to drop metadata or empty a live game it cannot explain.")

    main = 960
    side = 420

    c.start(main, 170)
    o1 = box(c, main, 205, 520, "run: pacenotes-server manifest gen --data ./data", "CLI", tag="OPERATOR")
    a1 = box(c, main, 285, 560, "load the current manifest.json (only the two guards need it), then walk <data>/", "CLI")

    c.rrect(120, 365, 1560, 425, radius=10, fill=None, outline="#C3CCD9", width=1.2)
    c.dashed([(120, 365), (120, 786), (124, 790), (1676, 790), (1680, 786), (1680, 365)],
             "#C3CCD9", 1.2, 6, 5)
    c.tag(150, 365, "for each folder under <data>/", c.reg(11), Ink.muted, align="left")

    d1 = c.decision(main, 395, 360, "reserved, stray file, or no packages/?")
    s1 = pill(c, side, 405, 330, "skip, with a warning", Ink.muted)
    d2 = c.decision(main, 500, 360, "schema.toml present and parses?")
    s2 = pill(c, side, 510, 330, "ERROR: this game publishes nothing", Ink.bad)
    a2 = box(c, main, 585, 560, "hash the schema; a hash this build does not bundle is a warning about a pending rollout", "CLI")

    c.rrect(560, 660, 1120, 118, radius=8, fill=None, outline="#C3CCD9", width=1.2)
    c.tag(590, 660, "for each zip under each access directory", c.reg(11), Ink.muted, align="left")
    d3 = c.decision(main, 682, 320, "category declared?")
    s3 = pill(c, side, 692, 300, "skip, with a warning", Ink.muted)
    a3 = box(c, main, 700, 600, "read <name>.toml (unknown key = error), then sha256, size and mtime", "CLI")

    a4 = box(c, main, 815, 620, "validate requires: the target exists, in the same game; public never needs protected content", "CLI")
    a5 = box(c, main, 900, 620, "summarise the last 30 days of conflict reports into per-pair warnings", "REPORT")
    d4 = c.decision(main, 990, 380, "would a live game vanish?")
    s4 = pill(c, side, 1000, 380, "REFUSE: exit 1, nothing written", Ink.bad)
    d5 = c.decision(main, 1085, 380, "would metadata be lost?")
    s5 = pill(c, side, 1095, 380, "REFUSE: exit 1, nothing written", Ink.bad)
    a6 = box(c, main, 1165, 620, "write manifest.json atomically: a temp file in the data dir, then rename", "CLI")
    a7 = box(c, main, 1240, 620, "print every published package; exit 0, or 2 when something was withheld", "CLI")
    c.end(main, 1330)

    c.arrow([(main, 181), (main, o1[1])])
    c.arrow([(main, o1[1] + o1[3]), (main, a1[1])])
    c.arrow([(main, a1[1] + a1[3]), (main, d1[1])])
    c.arrow([(d1[0], d1[1] + d1[3] / 2), (s1[0] + s1[2], d1[1] + d1[3] / 2)],
            label="yes", label_at=0.5, label_offset=(0, -12))
    c.arrow([(main, d1[1] + d1[3]), (main, d2[1])])
    c.arrow([(d2[0], d2[1] + d2[3] / 2), (s2[0] + s2[2], d2[1] + d2[3] / 2)],
            label="no", label_at=0.5, label_offset=(0, -12))
    c.arrow([(main, d2[1] + d2[3]), (main, a2[1])])
    c.arrow([(main, a2[1] + a2[3]), (main, d3[1])])
    c.arrow([(d3[0], d3[1] + d3[3] / 2), (s3[0] + s3[2], d3[1] + d3[3] / 2)],
            label="no", label_at=0.5, label_offset=(0, -12))
    c.arrow([(main, d3[1] + d3[3]), (main, a3[1])])
    c.arrow([(main, a3[1] + a3[3]), (main, a4[1])])
    c.arrow([(main, a4[1] + a4[3]), (main, a5[1])])
    c.arrow([(main, a5[1] + a5[3]), (main, d4[1])])
    c.arrow([(d4[0], d4[1] + d4[3] / 2), (s4[0] + s4[2], d4[1] + d4[3] / 2)],
            label="yes", label_at=0.5, label_offset=(0, -12))
    c.arrow([(main, d4[1] + d4[3]), (main, d5[1])])
    c.arrow([(d5[0], d5[1] + d5[3] / 2), (s5[0] + s5[2], d5[1] + d5[3] / 2)],
            label="yes", label_at=0.5, label_offset=(0, -12))
    c.arrow([(main, d5[1] + d5[3]), (main, a6[1])])
    c.arrow([(main, a6[1] + a6[3]), (main, a7[1])])
    c.arrow([(main, a7[1] + a7[3]), (main, 1317)])

    c.note(60, 1360, 560,
           "The two refusals are why an upgrade cannot silently empty the catalogue: a server whose "
           "data root lost schema.toml would otherwise regenerate an EMPTY manifest and serve it. "
           "--accept-unpublish and --accept-metadata-loss make the operator say so out loud.",
           font_size=11)
    c.note(640, 1360, 500,
           "The manifest is a PURE function of zips, sidecars and schemas. Nothing is inherited from "
           "the previous one, so deleting a sidecar key removes it - which is also why the metadata "
           "guard exists.", font_size=11)
    c.note(1180, 1360, 460,
           "Exit codes: 0 everything published, 2 written but something withheld (ERROR lines), "
           "1 could not run - and in that case nothing was written.", font_size=11)
    return c.save(os.path.join(path, "activity-manifest-gen.png"))


if __name__ == "__main__":
    print(server_request(OUT))
    print(manifest_gen(OUT))