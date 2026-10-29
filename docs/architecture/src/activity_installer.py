"""UML activity diagrams for the installer.

A1: one run of the wizard, end to end, with its three routes.
A2: what the worker does inside scene 6 (the install engine).

Run from the repository root:
    python docs/architecture/src/activity_installer.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from diagramkit import Ink, Kit  # noqa: E402

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

# (fill, edge) per participant. The tag on each action names the partition, so
# a reader does not have to decode colour alone.
WHO = {
    "UI": ("#EAF1FA", "#7FA0C6"),
    "WORKER": ("#F1EAF9", "#9A7FC0"),
    "SERVER": ("#E7F4EE", "#5FA183"),
    "DISK": ("#EEF1F5", "#8A97A8"),
    "USER": ("#FBF0E4", "#C08A55"),
    "ENGINE": ("#FDF2E7", "#C99A5B"),
    "LEDGER": ("#F3EEF9", "#9A7FC0"),
    "GAME FOLDER": ("#EEF1F5", "#8A97A8"),
}


def box(c, cx, y, w, text, who, tag=None, note=None):
    fill, edge = WHO[who]
    return c.action(cx, y, w, text, tag=tag if tag is not None else who, accent=edge, fill=fill)


def down(c, a, b, label=None, color=Ink.line):
    """Arrow from the bottom of box a to the top of box b."""
    cx = (a[0] + a[2] / 2 + b[0] + b[2] / 2) / 2
    c.arrow([(cx, a[1] + a[3]), (cx, b[1])], color=color, label=label, label_at=0.5,
            label_offset=(0, 0), label_font=c.reg(12))


def installer_run(path):
    c = Kit(1900, 1830)
    c.title(40, 22, "A1 — Installer: one run, five scenes and three routes",
            "An activity diagram of a single execution. The tag on each action names the partition that "
            "performs it: the wizard (UI thread), the one worker thread, the server, and the disk. "
            "Decisions are diamonds; the route choice on scene 3 is the only branch that skips pages.")

    spine = 930
    left = 380
    right = 1480

    c.start(spine, 120)
    a1 = box(c, spine, 150, 520, "Process start: single-instance mutex, common controls, OS language", "UI")
    a2 = box(c, spine, 230, 520, "Scene 1 — pick the UI language (default: the Windows one)", "UI")
    a3 = box(c, spine, 310, 560, "Scene 2 — message of the day, optional access key", "UI")
    d1 = c.decision(spine, 400, 400, "key typed?")
    a4 = box(c, right, 405, 380, "GET /whoami on the worker: 200 = live, 401 = wrong/expired/revoked", "WORKER")
    a5 = box(c, spine, 530, 560, "Scene 3 — pick the folder; the game is detected from its marker .exe", "UI")
    d2 = c.decision(spine, 630, 360, "route?")

    # --- Download / Update -------------------------------------------------
    b1 = box(c, left, 780, 420, "Scene 4 — tick what to have. Tree = catalogue + cache", "UI")
    b2 = box(c, left, 875, 420, "Schema-hash gate: an unknown hash blocks downloading for THIS game only", "UI")
    b3 = box(c, left, 990, 420, "Scene 4b — fetch the ticked packages: worker, sha256 verified as it streams", "WORKER")
    b4 = box(c, left, 1105, 420, "Cache gains <sha>.zip + cache.json; the game folder is still untouched", "DISK")

    # --- Configure / Install ----------------------------------------------
    c1 = box(c, spine, 780, 440, "Build the tree from cache + ledger: no server, no transfers", "UI")
    c2 = box(c, spine, 875, 440, "Everything already installed arrives ticked", "UI")

    # --- Restore / Uninstall ----------------------------------------------
    r1 = box(c, right, 780, 420, "Restore scene — points newest first, plus 'remove everything'", "UI")
    r2 = box(c, right, 875, 420, "Pick one: preview remove / put back / keep / unavailable", "UI")
    d3 = c.decision(right, 975, 320, "plan empty?")
    r3 = box(c, right, 1090, 380, "Nothing to do; stay on the preview", "UI")

    # --- shared tail -------------------------------------------------------
    s5 = box(c, spine, 1200, 560, "Scene 5 — one pick per choice category, plus the optional extras", "UI")
    d4 = c.decision(spine, 1340, 340, "every question answered?")
    n1 = box(c, left, 1350, 340, "Name the missing lists and stay", "UI")
    s6 = box(c, spine, 1460, 640,
             "Scene 6 — Install (worker): plan, pre-flight, restore point, uninstall, "
             "back up, extract, game step, report overlaps", "WORKER")
    s7 = box(c, spine, 1590, 560, "Scene 7 — summary: what changed, and where the log is", "UI")
    end = (spine, 1710)
    c.end(*end)

    # --- edges -------------------------------------------------------------
    c.arrow([(spine, 131), (spine, a1[1])])
    down(c, a1, a2)
    down(c, a2, a3)
    down(c, a3, (d1[0], d1[1], d1[2], d1[3]))

    # key typed: yes -> whoami (right), no -> straight on
    c.arrow([(spine + d1[2] / 2, d1[1] + d1[3] / 2), (a4[0], d1[1] + d1[3] / 2)],
            label="yes", label_at=0.5, label_offset=(0, -12))
    c.arrow([(a4[0] + a4[2] / 2, a4[1] + a4[3]), (a4[0] + a4[2] / 2, 525), (a5[0] + a5[2], 525),
             (a5[0] + a5[2], 545)])
    c.text(a4[0] + 6, a4[1] + a4[3] + 8, "401 → stay on scene 2 · unreachable → carry on unvalidated",
           c.reg(11), Ink.muted)
    c.arrow([(spine, d1[1] + d1[3]), (spine, a5[1])], label="no (anonymous)")

    down(c, a5, (d2[0], d2[1], d2[2], d2[3]))

    # route: three branches + cancel
    left_x, left_y = (d2[0], d2[1] + d2[3] / 2)
    c.arrow([(left_x, left_y), (b1[0] + b1[2] / 2, left_y), (b1[0] + b1[2] / 2, b1[1])],
            label="Download / Update", label_at=0.62, label_offset=(0, -13))
    c.arrow([(spine, d2[1] + d2[3]), (spine, c1[1])], label="Configure / Install",
            label_offset=(0, 0))
    rx, ry = (d2[0] + d2[2], d2[1] + d2[3] / 2)
    c.arrow([(rx, ry), (r1[0] + r1[2] / 2, ry), (r1[0] + r1[2] / 2, r1[1])],
            label="Restore / Uninstall", label_at=0.62, label_offset=(0, -13))
    c.note(60, 380, 420,
           "Cancel is the sheet's button, not a route: on any page before scene 6 it asks for "
           "confirmation and exits the run. Scene 6 keeps it disabled, because from there every "
           "phase writes to the game folder and the engine has no stop between files.",
           font_size=12)

    # download column
    down(c, b1, b2)
    down(c, b2, b3)
    down(c, b3, b4)
    # configure column
    down(c, c1, c2)
    # restore column
    down(c, r1, r2)
    c.arrow([(r2[0] + r2[2] / 2, r2[1] + r2[3]), (r2[0] + r2[2] / 2, d3[1])])
    c.arrow([(d3[0] + d3[2] / 2, d3[1] + d3[3]), (d3[0] + d3[2] / 2, r3[1])], label="yes",
            label_offset=(16, 0))

    # routes rejoin: the download route and the configure route both reach scene 5
    c.arrow([(b4[0] + b4[2], b4[1] + b4[3] / 2), (620, b4[1] + b4[3] / 2), (620, s5[1] + 30),
             (s5[0], s5[1] + 30)], label="fetched", label_at=0.62, label_offset=(0, -12))
    c.arrow([(c2[0] + c2[2] / 2, c2[1] + c2[3]), (c2[0] + c2[2] / 2, s5[1])])

    down(c, s5, (d4[0], d4[1], d4[2], d4[3]))
    c.arrow([(d4[0], d4[1] + d4[3] / 2), (n1[0] + n1[2], d4[1] + d4[3] / 2)],
            label="no", label_at=0.5, label_offset=(0, -12))
    c.arrow([(n1[0] + n1[2] / 2, n1[1]), (n1[0] + n1[2] / 2, s5[1] + s5[3] - 10),
             (s5[0], s5[1] + s5[3] - 10)], color=Ink.line, dash=5)

    # restore route joins scene 6, below the questions it skipped
    c.arrow([(d3[0] + d3[2], d3[1] + d3[3] / 2), (1735, d3[1] + d3[3] / 2), (1735, 1515),
             (s6[0] + s6[2], 1515)])
    c.text(1748, 1280, "a restore skips the questions", c.reg(11), Ink.muted, anchor="ma")

    down(c, (s6[0], s6[1], s6[2], s6[3]), s7)
    c.arrow([(spine, s7[1] + s7[3]), (spine, end[1] - 13)])

    # notes
    c.note(40, 1480, 480,
           "Scene 4's ticks are what the user wants to HAVE; scene 5's picks are what to USE. That "
           "split is why the download phase comes first — and why the install page never reaches the "
           "network except to report a conflict.", font_size=12)
    c.note(40, 1660, 480,
           "The install-schema check is a compatibility gate, not a kill switch: an unknown hash blocks "
           "DOWNLOADING for that game only. Reconfiguring, uninstalling and restoring keep working, "
           "because none of them asks the server anything.", font_size=12)

    c.legend(40, 1770, [("#7FA0C6", "wizard (UI thread)"), ("#9A7FC0", "worker thread"),
                        ("#5FA183", "server"), ("#8A97A8", "game folder / cache")], columns=4)
    return c.save(os.path.join(path, "activity-installer.png"))


def engine_run(path):
    c = Kit(1800, 1520)
    c.title(40, 22, "A2 — Installer: what scene 6 actually does (the install engine)",
            "The same run, opened up. Partitions are vertical swimlanes: the worker calls, the engine "
            "decides, the ledger records ownership, the game folder changes. Dashed regions repeat per "
            "package. Nothing here reaches the network except the optional conflict report at the end.")

    lanes = [("WORKER", 60, 330), ("ENGINE", 390, 560), ("LEDGER", 950, 400), ("GAME FOLDER", 1350, 390)]
    top = 120
    for name, x, w in lanes:
        c.rrect(x, top, w, 1330, radius=12, fill="#FAFBFD", outline=Ink.rule, width=1.2)
        c.text(x + 16, top + 12, name, c.semi(15), Ink.muted)
        c.dashed([(x + w, top + 44), (x + w, top + 1326)], Ink.rule, 1.2, 8, 7)

    # expansion regions, drawn first so the actions sit on top of them
    for y0, y1, label in [
        (752, 872, "for each package to remove"),
        (884, 1058, "for each package to install"),
        (1068, 1152, "the game's own step, once"),
    ]:
        c.rrect(420, y0, 1310, y1 - y0, radius=10, fill=None, outline="#C3CCD9", width=1.2)
        c.dashed([(420, y0), (420, y1 - 14), (424, y1), (1730 - 4, y1), (1730, y1 - 4), (1730, y0)],
                 "#C3CCD9", 1.2, 6, 5)
        c.tag(452, y0, label, c.reg(11), Ink.muted, align="left")

    start = (225, 180)
    c.start(*start)
    w1 = box(c, 225, 210, 270, "open ledger + cache; build the available map", "WORKER")
    e1 = box(c, 670, 300, 470,
             "plan(selection, ledger, cache): what to uninstall (dropped, superseded, half-written, "
             "sha changed) and what to install", "ENGINE")
    e2 = box(c, 670, 410, 470, "pre-flight: payload sha256, writable probe, free space, every file list", "ENGINE")
    d1 = c.decision(670, 520, 420, "plan and pre-flight pass?")
    e3 = box(c, 670, 640, 470, "overlaps(): files two selected packages both write", "ENGINE")
    e4 = box(c, 670, 780, 470, "execute(): uninstalls first, always", "ENGINE")
    l1 = box(c, 1150, 700, 300, "open restore point: <stamp>\\ with ledger.json as it is now", "LEDGER")
    l2 = box(c, 1150, 780, 300, "per file: another owner? hash still ours? prior said delete, copy back, or leave", "LEDGER")
    g1 = box(c, 1545, 780, 300, "file deleted, or the original copied back", "GAME FOLDER")
    e5 = box(c, 670, 925, 470, "per package: classify_prior (backup | identical | absent)", "ENGINE")
    l3 = box(c, 1150, 925, 300, "put(state = installing) — the write-ahead entry", "LEDGER")
    g2 = box(c, 1545, 925, 300, "extract: temp file + rename, one file at a time", "GAME FOLDER")
    l4 = box(c, 1150, 1000, 300, "put(state = installed) with each file's sha256", "LEDGER")
    e6 = box(c, 670, 1085, 470, "game step: RBR edits language + sounds in PaceNote.ini, and nothing else", "ENGINE")
    l5 = box(c, 1150, 1085, 300, "record_configured(): adopt the step's edits, or they look foreign", "LEDGER")
    w2 = box(c, 225, 1180, 270, "report overlaps: POST /report/conflict (a failure is only logged)", "WORKER")
    endp = (225, 1300)
    c.end(*endp)
    fail = (225, 1400)
    c.end(*fail)
    c.text(255, 1390, "failed: scene 7 shows the message", c.reg(11), Ink.muted)

    # flow
    c.arrow([(225, 191), (225, w1[1])])
    c.arrow([(w1[0] + w1[2], w1[1] + w1[3] / 2), (e1[0], w1[1] + w1[3] / 2)])
    down(c, e1, e2)
    down(c, e2, (d1[0], d1[1], d1[2], d1[3]))
    down(c, (d1[0], d1[1], d1[2], d1[3]), e3, label="yes")
    down(c, e3, e4)
    c.arrow([(e4[0] + e4[2], e4[1] + 14), (1116, e4[1] + 14), (1116, l1[1] + 30), (l1[0], l1[1] + 30)],
            label="before anything moves", label_at=0.28, label_offset=(-58, -12))
    c.arrow([(e4[0] + e4[2], e4[1] + e4[3] / 2), (l2[0], e4[1] + e4[3] / 2)])
    c.arrow([(l2[0] + l2[2], l2[1] + l2[3] / 2), (g1[0], l2[1] + l2[3] / 2)])
    down(c, e4, e5)
    c.arrow([(e5[0] + e5[2], e5[1] + e5[3] / 2), (l3[0], e5[1] + e5[3] / 2)],
            label="write-ahead", label_at=0.5, label_offset=(0, -12))
    c.arrow([(l3[0] + l3[2], l3[1] + l3[3] / 2), (g2[0], l3[1] + l3[3] / 2)])
    c.arrow([(g2[0] + g2[2] / 2, g2[1] + g2[3]), (g2[0] + g2[2] / 2, 1040), (l4[0] + l4[2], 1040)])
    down(c, l3, l4)
    down(c, e5, e6)
    c.arrow([(e6[0] + e6[2], e6[1] + e6[3] / 2), (l5[0], e6[1] + e6[3] / 2)])
    c.arrow([(l4[0], l4[1] + l4[3] / 2), (1116, l4[1] + l4[3] / 2), (1116, e6[1] + e6[3] / 2),
             (e6[0] + e6[2], e6[1] + e6[3] / 2)], dash=5, label="then", label_at=0.62,
            label_offset=(0, -12))
    down(c, e6, w2)
    c.arrow([(w2[0] + w2[2] / 2, w2[1] + w2[3]), (225, endp[1] - 13)])

    # a failed plan or pre-flight stops here, before any file moves
    c.arrow([(d1[0], d1[1] + d1[3] / 2), (405, d1[1] + d1[3] / 2)], label="no", label_at=0.5,
            label_offset=(0, -12))
    c.arrow([(405, d1[1] + d1[3] / 2), (405, 1400), (fail[0], 1400)])
    c.text(250, 1330, "missing requirement · full disk · payload mismatch", c.reg(11), Ink.muted)

    c.note(40, 300, 250,
           "The engine is pure std and testable off Windows: free space and 'is the game running' are "
           "measured by the UI thread and passed in as HostFacts.", font_size=11)
    c.legend(40, 1460, [("#C99A5B", "engine decision"), ("#9A7FC0", "ledger write"),
                        ("#8A97A8", "file system effect")], columns=3)
    return c.save(os.path.join(path, "activity-engine.png"))


if __name__ == "__main__":
    print(installer_run(OUT))
    print(engine_run(OUT))