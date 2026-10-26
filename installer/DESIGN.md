# Installer UI design — wizard flow

The installer downloads, caches and installs mod packages for one game per
run. Supported games, detected by their marker executable:

| Game | Marker executable | Game id (`pacenotes_shared::GAME_*`) |
| --- | --- | --- |
| Richard Burns Rally | `RichardBurnsRally_SSE.exe` | `rbr` |
| Dirt Rally 2.0 | `dirtrally2.exe` | `dirtrally2` |
| Assetto Corsa | `assettocorsa.exe` | `assettocorsa` |
| BeamNG | `BeamNG.drive.exe` | `beamng` |

## Flow

Eight pages, and TWO ROUTES through them. Scene 3 is where they part:

    1 Language → 2 Message of the day → 3 Game folder
                                          │
            Download/Update ──────────────┤
                                          │
      4 Download → 4b Downloading → 5 Configuration → 6 Installation → 7 Summary
                                          ▲
            Configure/Install ────────────┘

- **Download/Update**: choose content, fetch it into the cache, THEN answer
  the configuration questions, THEN install.
- **Configure/Install**: skip both download pages. The questions are answered
  against what this computer already holds (the cache) and what this folder
  already has (the ledger).

Downloading and installing are deliberately two screens with the
configuration step BETWEEN them, rather than one progress screen that did
everything:

- The download only fills a cache; the install writes to the game folder.
  Someone can fetch a voice pack at home and install it on a train.
- A user who only wants to change which co-driver is active should never
  wait for, or depend on, the server. That is the whole Configure/Install
  route, and it is the common case after the first run.
- The questions are easier to answer once the content is actually here: a
  list built from the cache cannot offer something that failed to download.

Common chrome on every page: a title, the content area, and a footer with
**Back** (scenes 3–5 only), the page's **primary button** (its label differs
per page) and **Cancel** (asks for confirmation; disabled on scene 6, where
every phase writes to the game folder). No going back once scene 4b starts.

The window is ONE size for every scene — a property sheet sizes itself to its
largest page — so a scene with little content, such as scene 1, simply has
space around it. That size is 330x215 DLU, set by scene 5: three lists have
to be visible at once, and each at full width, because a co-driver is
identified by its whole path and a narrower column clips exactly the part
that tells two voices apart.

## Scene 1 — Language

- Language combobox + **Continue**.
- Default selection = OS UI language if supported, else English.
- The choice applies to the whole run (`SetThreadUILanguage`). No language
  switch later; restart the app to change it.
- This scene is shown BEFORE a language has been chosen, so it necessarily
  renders in the OS UI language.

## Scene 2 — Message of the day

- Read-only multi-line text: the MOTD from the server (`GET /motd?lang=xx`,
  English fallback). Offline → built-in localized text saying the server is
  unreachable and only cached content is available. A 404 from the server
  (no MOTD published) is treated exactly like being offline.
- **Secret** field (password style). The secret unlocks extra (private)
  content: it is the bearer access key for every later server request.
- Bottom line: **Server status: Online / Offline**, from `GET /health` on the
  worker thread when the scene opens ("Checking…" until it answers).
- **ACCEPT**:
  - empty secret → continue anonymously;
  - secret + online → validate with `GET /whoami`; 401 → warning modal
    "invalid secret", stay on the scene;
  - offline → continue (secret kept, unvalidated).
- `GET /whoami`, not `GET /packages`: `/packages` never answers 4xx — it
  silently falls back to the public listing for a key it does not accept,
  so directory names cannot be enumerated, which also made a mistyped key
  look exactly like a good one. Anything other than a clean 401 (a dead
  server, a 200 we cannot parse) counts as "could not ask" and continues
  with the secret unvalidated; locking a user out of content they paid for
  over a protocol change would be the worse failure.
- `/whoami` also returns when the key expires, and a key within a fortnight
  of that says so in an informational modal with the date. A key that
  lapses between two runs otherwise looks exactly like content being
  withdrawn.

## Scene 3 — Game folder

- Path edit + **Browse** (folder picker) + **game dropdown**.
- Dropdown starts disabled showing the placeholder **(Detecting)**.
- After Browse returns, and on path edit (debounced ~400 ms), scan the
  folder for the marker executables:
  - exactly one match → dropdown shows that game (disabled: nothing to pick);
  - several matches → dropdown enabled, listing only the matched games;
  - no match → dropdown back to (Detecting).
- **Warning modal** when no marker is found — after Browse returns and when
  a route button is pressed, never per keystroke.
- **Three route buttons** in two columns, all enabled only when a game is
  detected/selected, with a line above them saying what each one does:

      ┌───────────────────┐      ┌─────────────────────────┐
      │ Configure/Install │      │                         │
      ├───────────────────┤      │     Download/Update     │
      │ Restore/Uninstall │      │                         │
      └───────────────────┘      └─────────────────────────┘

  - **Download/Update** → scene 4. The MAIN flow, and the shape says so:
    the default button, the widest, and as tall as the other two together,
    alone on the right where the eye ends up and where the sheet's own Next
    sits.
  - **Configure/Install** → scene 5, skipping both download pages.
  - **Restore/Uninstall** → the restore scene below.
  - The two secondary ones are work on a folder you already have, so they
    stack in a narrower column on the left. The stack's top and bottom are
    exactly the big button's, so the three read as one block, and the gap
    down the middle is what separates secondary from primary.
- RECORDED DEVIATION: these are buttons IN THE PAGE, and the sheet's own
  Next is left disabled on this scene. They are alternatives rather than one
  primary and two afterthoughts, each is one click instead of
  select-then-Next, and three routes do not fit on one Next anyway. Back and
  Cancel are unaffected.
- The run log moves into this folder here (`.pacenotes\logs\`), replaying
  the lines from before a folder was known, and later panics are written to
  the same file. Pressing a second route button does NOT start a second log:
  one file per run. See "Logging".

## Restore scene (reached from scene 3's Restore/Uninstall)

NOT BUILT YET: the button is there and answers "not implemented yet". It
ships ahead of the scene on purpose — the folder page's shape should not
change under users the day the scene lands, and a user who is looking for a
way back needs to see that one is coming.

The case this exists for: **a user has been handed a bad package.** They do
not want the installer gone, they want yesterday back. So Restore is "take
this game folder back to how it was at a point in time", not "copy some
files back".

- A list of **restore points**, newest first, each showing its date and
  what it was: `2026-11-02 18:15 — before installing Kamil 3.0.0`. A run
  that changed nothing creates none.
- The selected point shows a **preview before anything happens** — the
  whole point of restoring after a bad package is knowing what you are
  getting back:

      Restoring to 2026-10-21 09:40 would:
        put back    3 files replaced since then
        remove      2 packages installed since then (1,204 files)
        keep        Kamil 2.1.0, Descriptive, polish

- **Restore** then runs on the progress scene (scene 6) like any other
  phase, with the same log.

Restoring is TWO operations, and the second is the one the legacy installer
never had:

1. **Put back** the files replaced since that point, taking each from the
   newest restore point at or after it that holds the file, so the oldest
   version wins.
2. **Remove** what was installed after that point. Copying files back
   cannot do this: a broken package that ADDED files leaves every one of
   them behind, and the user still has the broken install. This needs to
   know what was installed and when, which is what the ledger snapshot
   inside each restore point is for.

A restore point is therefore `.pacenotes\restore\<timestamp>\` holding the
replaced files plus `ledger.json` as it was before that run. Restoring
replaces the live ledger with that snapshot, so the installer's own record
matches the folder afterwards — otherwise the next run would "uninstall"
packages that are no longer there and reinstall ones that are.

Points are never pruned automatically. They are small (only files actually
replaced, and never the byte-identical ones), and the one a user needs is
the one an automatic policy would have deleted.

## Scene 4 — Download

- One tree view with checkboxes, multi-level:

      [ ] STYLE
          [ ] FILE_NAME
      [ ] LANGUAGE
          [ ] FILE_NAME
      [ ] PACENOTES
          [ ] FILE_NAME
      [ ] CODRIVER
          [ ] LANGUAGE
              [ ] GENDER
                  [ ] FILE_NAME

  The shape comes from where each package SITS on the server:
  `PackageInfo.name` is its path under the access directory, so
  `codriver/polish/male/Kamil` gives level 1 = `category` (`codriver`),
  middle levels (`polish`, `male`) and the leaf (`Kamil`).
- Only packages for the detected game are shown; the manifest is fetched with
  `?game=<id>` so the server filters rather than the client.
- Loading order:
  1. server manifest (with the secret, if any);
  2. merge the download cache: downloaded leaves get the suffix
     "(downloaded)" and are checked by default; cached packages not on the
     server are added with "(local only)"; cached packages with a newer
     server version get "(update)".
- Offline: tree built from the cache only.
- Checking a parent checks all its descendants; mixed descendants show the
  parent as partially checked.
- Unchecking a downloaded leaf only excludes it from this run; it is not
  removed from the cache.
- **Download**: requires at least one checked leaf in every **choice**
  category the game declares; otherwise a warning modal names the missing
  ones. Any other category the operator publishes — `pacenotes`, or a new
  `extras/` folder — is optional, because adding a folder on the server
  must not add a question every user is forced to answer.
- Nothing here gates `exclusive_group`. Two members of one group can be
  ticked and both will install, the later winning; see "Overlapping files"
  in `installer/CLAUDE.md` for why that is preferred to blocking the user.
- The button commits the selection; the transfers themselves run on scene
  4b, which this page walks straight on to.
- **If the game's install schema is not one this build knows**, Download
  stops with "a newer installer is needed", because the catalogue may use
  categories this build does not know how to place. The manifest carries
  the SHA-256 of each game's schema file (raw bytes) and the installer
  compares it with the copy it embeds from `schemas/<game>.toml`; see
  "Install-schema check" in `installer/CLAUDE.md`.
  - It blocks downloading for THAT GAME only, and only downloading.
    Reconfiguring, repairing, uninstalling and restoring what is already
    on the machine stay available — none of them consults the server.
  - The warning is raised by the Download button, not at startup, and only
    when something actually needs fetching.
  - An offline run never raises it: the check runs on a manifest that came
    from the server, not on a tree rebuilt from the cache.

## Scene 4b — Downloading

The same screen as scene 6 — progress bar, phase label, current item with
size, log pane — at an earlier moment, and a page of its own because what
happens between the two is the user answering the configuration questions.

- Starts itself on activation and needs no button: it fetches everything
  TICKED on scene 4 that is not already in the cache, verifies each payload
  against the manifest's sha256, and walks on to scene 5 when done.
- Nothing to fetch (a re-run, or everything already cached) is the normal
  case, not an edge: the page says so in the log and moves on.
- **Nothing is written to the game folder here.** That is what makes the
  split worth having, and what the test asserts by checking there is no
  ledger when scene 5 appears.
- **Cancel** is allowed, and this is the only phase where it is: a
  half-finished download costs nothing and is discarded.
- A failed or cancelled download ends the run at scene 7 rather than
  continuing to the questions. There is nothing to configure that could
  then be honestly installed.
- Back is not offered; the configuration scene's Back returns to scene 4,
  skipping this page, because there is no going back into a finished
  transfer.

## Scene 5 — Configuration

- Three single-select lists, filled from leaves checked in scene 4:

  | List | Category |
  | --- | --- |
  | Pacenotes Language | `language` |
  | CoDriver Voice | `codriver` (shown with its path, e.g. "polish / male / KamilFilipek") |
  | Pacenotes Style | `style` |

The three lists are not a fixed set: they are the categories this build's
schema for the game marks `select = "one"`, in schema order
(`schema::choice_categories`). The localized label and one-line explanation
of each come from the `.rc` string tables, mapped by category id — never
from the server. A game whose schema declares no single-pick category skips
this scene.

- **What this folder already has installed is the default answer**, which is
  what makes the Configure/Install route usable: that route exists to CHANGE
  an install, so arriving with every question blank would make the user
  re-answer what they answered last time — and a run whose selection omitted
  an installed package would UNINSTALL it. A pick the user makes themselves
  wins over it.
- A list with exactly one option is preselected.
- On the Configure/Install route the options come from the cache and the
  ledger alone; the page is otherwise identical, because it reads the same
  tree either way.
- Checked `pacenotes`-category packages are installed as-is (no choice), and
  so is anything under a category the installer does not know.
- **Install** requires a pick in every list that HAS options; a warning modal
  names the ones still unanswered. A category the server publishes nothing
  under leaves an empty, disabled list that says so — scene 4 does not demand
  a tick there either, and dead-ending the wizard on a catalogue the operator
  is entitled to publish would be worse than installing without it.
- Going Back rebuilds the lists. A pick still on offer is kept; one whose
  package was unticked is dropped and the question is asked again. Back
  leads to scene 4 on the Download route and to scene 3 on the
  Configure/Install route — where the user came from, which on neither
  route is the previous page.

## Scene 6 — Installation

- Overall progress bar, phase label, current file with size
  ("Numeric 1to7.zip — 1.2 / 4.8 MB"), live log pane. Same layout as scene
  4b; the download phase is no longer part of this page.
- Phases, in order:
  1. **Check** the plan against the folder (free space, CRCs);
  2. **Uninstall** what a new pick supersedes — before anything is written,
     because the style packages all own the SAME 49 paths and uninstalling
     after installing would delete the new one's files;
  3. **Backup** files the install will overwrite and the installer does not
     already own (first touch only, under `.pacenotes\backup\` in the game
     folder), skipping any whose bytes already match what will replace them;
  4. **Install** the three configured packages + checked `pacenotes`
     packages;
  5. **Configure** — the chosen game's own step, if it has one. RBR's sets
     the pacenotes language in the Pacenote plugin's config. Other games get
     their own step or none; see "Per-game configuration" below.
- Everything this page installs from is already in the cache: scene 4b saw
  to that, or the user took the Configure/Install route because it was
  already there. The page therefore works with the server unreachable.
- Backup needs each package's file list, which comes from reading the
  package's own zip — another reason the download has to have finished
  before this page starts.
- Scene 4b downloaded everything TICKED on scene 4, not just the scene 5
  picks. The user ticked what they want to have; they chose separately what
  to install.
- Files that more than one selected package writes do NOT stop the run: the
  later package wins, the log says which files and which packages, and the
  installer tells the server (`POST /report/conflict`, after the install, so
  what is reported is what happened). The operator is the only party who can
  fix a catalogue like that, and a user must never be blocked by a mistake
  they cannot fix. The report carries the game, this installer's version and
  the clashing `directory/name` refs in install order — no identity, no local
  paths, no token — and failing to send it is a log line, nothing more.
- The scene has no buttons of its own: no Back (no going back once it
  starts) and no Next, because it walks on to the summary by itself when the
  run ends.
- **Cancel is disabled here.** Every phase on this page writes to the game
  folder and the engine has no way to stop between files; a button that
  silently did nothing would be worse than none. Cancel belongs to scene 4b,
  where the only thing in flight is a download.
- On error: stop, keep the log visible, go to scene 7 in failure state.

## Scene 7 — Summary

- Result: success / failure.
- What was done: game + folder, the three installed choices, `pacenotes`
  packages, number and total size of downloads, what the game's own
  configuration step changed, and the backup location — named only when
  something was actually backed up, since a run that replaced nothing of
  the user's has no backup to point at.
- Log pane: collapsed on success, expanded on failure. Collapsed means
  hidden with the detail block stretched over its room, not a smaller box —
  a successful run's log is noise and a failed one's is the point.
- Cancel is disabled here: the run is over, and **Finish** is the way out.
- **Finish** closes the app.

## Logging

Every scene appends to an in-memory log, also written to
`<game dir>\.pacenotes\logs\install-<utc stamp>.log`. The log is visible on
scenes 4b and 6 and again on 7; any error shows it.

- **In the game folder, not the user's profile.** The installer leaves state
  in exactly two places, and this is the one that belongs to a game: it sits
  with the ledger, the backups and the restore points it describes, so a
  user asking for help about a game folder has everything in one place and
  nothing is scattered through their profile. (The download cache stays in
  `%LOCALAPPDATA%`, because it is shared by every game folder.)
- **One file per run**, dated in the name. A single overwritten file meant
  that re-opening the installer to see what went wrong ERASED the run being
  looked at (DESIGN-REVIEW §7). The newest five are kept; a log is a few kB.
- The file is attached when scene 3 settles the folder, and the lines from
  before that — the language, the server, the catalogue — are replayed into
  it, so the file is the whole run and not just its second half.
- Every line is flushed as it is written: a run that dies still leaves the
  line saying what it was doing.
- **A panic is written down too.** This is a GUI-subsystem binary built
  with `panic = "abort"`, so a panic has nowhere to print: the process
  vanishes and Windows records an offset into a stripped exe, which tells
  nobody anything. A panic hook (`main.rs`) catches it first and writes
  `panicked at <file>:<line>: <message>` into the run log — or, before a
  game folder is known, into `pacenotes-installer-crash.log` beside the
  exe.
  - What the USER sees is localized and plain: something went wrong, where
    it was written, that the game folder is not half-installed, and
    **"Show that file?"** — which opens Explorer with the log selected. A
    report the user cannot find is one they cannot send. The Rust text
    stays in the file, for whoever reads the report.
  - The message is loaded after re-applying the chosen language, because
    the hook can run on the worker thread, which never had it set.
  - Only the FIRST panic gets a window. The modal pumps the message queue,
    which can deliver the very message that panicked, and one real fault
    was observed turning into a cascade of boxes.
  - It covers the worker thread as well: the hook is global, and with
    `panic = "abort"` a worker panic takes the process with it either way.
  - It does NOT cover access violations or other hardware faults in Win32
    code — those never reach a Rust panic hook.

## Implementation notes

### Installer state in the game folder

One directory, the same for every game:

    <game folder>\.pacenotes\installed.json      the ledger
    <game folder>\.pacenotes\backup\<rel path>   first-touch backups

Not inside the plugin tree. `Plugins\Pacenote\installer\` is where the C#
version kept it, but that path exists only for RBR, and for RBR it sits in
a directory that packages and the RSF installer also write to — something
else cleaning up there would take the ledger with it. One location means
one rule, no per-game table, and all installer state is one thing to find,
copy with the folder, or delete.

### Per-game configuration

After the install phase the chosen game may run a step of its own. There is
deliberately **no shared abstraction** for these: the games do not share a
file layout, several have no plugin system at all, and a step may have to
edit the game's own files rather than a plugin's config. A single
"ini key = value" mechanism would fit RBR and mislead everyone adding the
next game.

Each game therefore owns its step outright, and the only contract is the
plumbing: a step reports the files it wrote or changed, and those are
ledgered and backed up exactly like extracted ones, so uninstall and
restore need to know nothing about what the step did.

**RBR's step** modifies the Pacenote plugin's `Plugins\Pacenote\PaceNote.ini`:

- The co-driver packages already ship that file, complete, with
  `sounds=<the voice's own path>` and the author's own `language=`,
  `muteReplayPacenotes` and comments. So the step EDITS it — setting
  `language` to the user's pick, and `sounds` to the chosen voice — and
  leaves every other line exactly as the author wrote it.
- If the file is not there, it is written from a built-in template holding
  the plugin's documented defaults.
- If `Plugins\Pacenote\config\` is missing entirely, the plugin is not
  installed: the step reports that, as the C# version's message did, rather
  than writing config for something that is not there.

What it does NOT do is regenerate the file the way `CreateRBRConfiguration`
did. The packages have outgrown that: a regenerated file would discard each
voice's `muteReplayPacenotes`, the author's comments, and any key the plugin
gains later. The style half of that old function is obsolete outright — the
style packages now ship the `config\pacenotes\*.ini` files it used to write,
with the correct `file0=packages\FilipekMod_<style>.ini` per style.

### Wizard host

comctl32 **PropertySheet in wizard mode** (`PSH_WIZARD`): Back/Next/Cancel,
page switching and keyboard handling come from the OS at ~0 bytes of ours,
which matters against the 600 kB gate. One dialog template per scene in
`res/installer.rc`; button enablement via `PSM_SETWIZBUTTONS`.

Three things the OS does NOT give, each worked around in `wizard.rs` with the
reason at the call site:

- **The scene title is a static control in each page template**, not the
  Wizard97 header. Wizard97 draws its header title only alongside a header
  BITMAP; without one it reserves an empty band and ignores both
  `PROPSHEETPAGEW::pszHeaderTitle` and `PSM_SETHEADERTITLE`. Shipping a bitmap
  to satisfy it would spend size budget on decoration, and a static is
  localizable per scene anyway.
- **Per-scene button labels are written directly** to comctl32's own button
  ids (`ID_WIZNEXT` / `ID_WIZFINISH` from prsht.h). `PSM_SETNEXTTEXT` is
  documented for this and is ignored by the sheet. Back and Cancel are
  relabelled the same way, because comctl32 supplies them in the OS language
  and a Polish wizard would otherwise carry two English buttons.
- **The title bar is set with `WM_SETTEXT`** when each scene activates;
  `PROPSHEETHEADERW::pszCaption` has no effect.

### Localization

Per-language `STRINGTABLE`s in `res/installer.rc` plus `SetThreadUILanguage`,
so `LoadStringW` resolves the right table with no lookup code of our own.
Every caption is set from the string table at `WM_INITDIALOG`; the text in the
templates is an English fallback only.

### Tree

`SysTreeView32` + `TVS_CHECKBOXES`, partial state via
`TVS_EX_PARTIALCHECKBOXES`; downloaded leaves bold (`TVIS_BOLD`) + suffix.

### Threading

All network work on the worker thread (`worker.rs`); the UI thread is the only
one that touches controls. A worker posts a message and the UI thread acts on
it.

### Protocol

- `GET /packages?game=<id>` — the manifest for one game.
- `GET /download/{game}/{directory}/{*name}` — build the tail with
  `PackageInfo::download_path()`; never assemble it by hand. A name may need
  percent-encoding (e.g. `style/Numeric Standard`).
- `GET /motd?lang=xx`, `GET /health`.
- A package's identity is `game` + `directory` + `name`. "directory/name" is
  NOT unique across games, so the cache index and the install ledger must key
  on the game as well — see `installer/CLAUDE.md`.
