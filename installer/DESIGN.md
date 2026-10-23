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

A linear wizard of seven scenes:

    1 Language → 2 Message of the day → 3 Game folder → 4 Download
      → 5 Configuration → 6 Installation → 7 Summary

Common chrome on every scene: a title, the content area, and a footer with
**Back** (scenes 3–5 only), the scene's **primary button** (its label differs
per scene) and **Cancel** (asks for confirmation; disabled during the backup
and install phases of scene 6). No going back once scene 6 starts.

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
  - secret + online → validate with `GET /packages`; 401 → warning modal
    "invalid secret", stay on the scene;
  - offline → continue (secret kept, unvalidated).

## Scene 3 — Game folder

- Path edit + **Browse** (folder picker) + **game dropdown**.
- Dropdown starts disabled showing the placeholder **(Detecting)**.
- After Browse returns, and on path edit (debounced ~400 ms), scan the
  folder for the marker executables:
  - exactly one match → dropdown shows that game (disabled: nothing to pick);
  - several matches → dropdown enabled, listing only the matched games;
  - no match → dropdown back to (Detecting).
- **Warning modal** when no marker is found — after Browse returns and when
  Next is pressed, never per keystroke.
- **Next** enabled only when a game is detected/selected.
- **UNINSTALL** enabled only when a game is detected/selected; leads to the
  restore scene below.

## Restore scene (reached from scene 3's UNINSTALL)

- List of selectable previous backup packages.
- **Restore** enabled when a package is selected.
- Switches to the progress scene and restores everything in that backup.

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
- **Download**: requires at least one checked leaf in every top-level
  category present in the tree; otherwise a warning modal names the missing
  categories. The button commits the selection; transfers run in scene 6
  so there is a single progress screen.

## Scene 5 — Configuration

- Three single-select lists, filled from leaves checked in scene 4:

  | List | Category |
  | --- | --- |
  | Pacenotes Language | `language` |
  | CoDriver Voice | `codriver` (shown with its path, e.g. "polish / male / KamilFilipek") |
  | Pacenotes Style | `style` |

- A list with exactly one option is preselected.
- Checked `pacenotes`-category packages are installed as-is (no choice), and
  so is anything under a category the installer does not know.
- **Install** requires a pick in every list that HAS options; a warning modal
  names the ones still unanswered. A category the server publishes nothing
  under leaves an empty, disabled list that says so — scene 4 does not demand
  a tick there either, and dead-ending the wizard on a catalogue the operator
  is entitled to publish would be worse than installing without it.
- Going Back to scene 4 and changing the ticks rebuilds the lists. A pick
  still on offer is kept; one whose package was unticked is dropped and the
  question is asked again.

## Scene 6 — Installation

- Overall progress bar, phase label, current file with size
  ("Numeric 1to7.zip — 1.2 / 4.8 MB"), live log pane.
- Phases, in order:
  1. **Download** checked packages missing from the cache (sha256 verified);
  2. **Backup** files the install will overwrite and the installer does not
     already own (first touch only, under `backup\` in the game folder);
  3. **Install** the three configured packages + checked `pacenotes`
     packages; RBR additionally runs `write_rbr_config`.
- Backup runs AFTER download because it needs each package's file list, and
  that list comes from reading the package's own zip.
- Phase 1 downloads everything TICKED on scene 4 that is not already in the
  cache — not just the scene 5 picks. The user ticked what they want to have;
  they chose separately what to install.
- The scene has no buttons of its own: no Back (DESIGN.md allows no going
  back once it starts) and no Next, because it walks on to the summary by
  itself when the run ends.
- Cancel allowed during phase 1 only; partial downloads are discarded.
  Pressing it VETOES the window close: the flag is raised, the worker stops
  between chunks and reports it, and the wizard reaches scene 7 — rather
  than the window vanishing while a thread still holds its handle.
- On error: stop, keep the log visible, go to scene 7 in failure state.

## Scene 7 — Summary

- Result: success / failure.
- What was done: game + folder, the three installed choices, `pacenotes`
  packages, number and total size of downloads, backup location.
- Log pane: collapsed on success, expanded on failure. Collapsed means
  hidden with the detail block stretched over its room, not a smaller box —
  a successful run's log is noise and a failed one's is the point.
- Cancel is disabled here: the run is over, and **Finish** is the way out.
- **Finish** closes the app.

## Logging

Every scene appends to an in-memory log, also written to
`%LOCALAPPDATA%\PacenotesInstaller\install.log` (overwritten per run). The
log is visible on scenes 6 and 7; any error shows it.

## Implementation notes

### Wizard host

comctl32 **PropertySheet in wizard mode** (`PSH_WIZARD`): Back/Next/Cancel,
page switching and keyboard handling come from the OS at ~0 bytes of ours,
which matters against the 500 kB gate. One dialog template per scene in
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
