# Requirements — how work is done here

Specs and standing rules for carrying out `TASKS.md`. Per-component specs
live beside their component: `installer/requirements.md`,
`server/requirements.md`.

What belongs where, so nothing lands in two places:

| File | Holds |
|---|---|
| `TASKS.md` | WHAT to do, as a checklist. No prose. |
| `requirements.md` | WHAT a task must satisfy before it counts as done. |
| `CLAUDE.md` | Rules that constrain future code, and why they exist. |
| `installer/DESIGN.md` | How the product behaves, screen by screen. |
| `CHANGELOG.md` | What changed, when, and what it cost. |
| `.claude/session.md` | What is being worked on right now. |

## Global guidelines

- Follow the task lists top to bottom: `TASKS.md` for milestones, then
  `installer/TASKS.md` and `server/TASKS.md` for the detail.
- Tick items as they are finished. Work discovered while doing a task
  becomes a SUB-POINT of that task, not a new top-level one.
- Prefer `std` over a dependency everywhere, and especially in
  `installer/` — see the size gate below.
- A protocol change means `shared/` first, then both sides, in one commit.
- Never commit on the user's behalf. Hand work over for them to review.

## Verification & definition of done

Two bars. Which one applies depends on the state the task leaves the
application in — not on how large the change was.

### Fast loop — for a task that leaves the build in pieces

For work that knowingly breaks something and needs the FOLLOWING task to
make the application whole again:

```powershell
cd installer; cargo fmt; cargo clippy --all-targets; cargo test
```

All three must pass. `clippy` runs with `-D warnings` in CI, so treat a
warning as a failure here too.

### Full suite — for a task that leaves the application working

```powershell
pwsh -File installer/tests/Run-AllTests.ps1
```

Which is: fmt, clippy, the cargo test binaries, version sync, the 600 kB
size gate, the WinHTTP client against a live server, the crash report, the
UI layout in both languages, and the wizard flow end to end. It needs an
interactive desktop for the last three.

**Then stop and ask the user whether they are happy with the state of the
application. Make no fixes without explicit permission** — a failing check
is a finding to report, not an invitation to start changing things.

### Always

- The installer binary stays under **600 kB**. `Test-BinarySize.ps1` is
  part of the full suite; run it by hand after anything that adds code or
  resources.
- A change to behaviour updates `installer/DESIGN.md`; a change to a rule
  updates the matching `CLAUDE.md`; both get a `CHANGELOG.md` entry.

## Decisions

When a task reaches a fork with no obviously right answer — a trade-off
between safety and convenience, two defensible designs, anything that
changes what a user sees — STOP, lay the options out with their costs, and
wait for an answer. Do not pick one and carry on.
