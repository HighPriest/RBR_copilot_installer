# Installer UI design (rev. 2 — wizard flow)

The installer downloads, caches and installs mod packages for one game per
run. Supported games, detected by their marker executable:

| Game | Marker executable | Game id (`pacenotes_shared::GAME_*`) |
| --- | --- | --- |
| Richard Burns Rally | `RichardBurnsRally_SSE.exe` | `rbr` |
| Dirt Rally 2.0 | `dirtrally2.exe` | `dirtrally2` |
| Assetto Corsa | `assettocorsa.exe` | `assettocorsa` |
| BeamNG | `BeamNG.drive.exe` | `beamng` |

Rev. 2 replaces the rev. 1 layout (game ribbon, two columns, Download/Backups
tabs, Notes/Logs tabs).

## Flow

A linear wizard of seven scenes:

    1 Language → 2 Message of the day → 3 Game folder → 4 Download
      → 5 Configuration → 6 Installation → 7 Summary

Common chrome on every scene: title, content area, footer with **Back**
(scenes 3–5 only), the scene's **primary button** (label per scene) and
**Cancel** (asks for confirmation; disabled during the backup and install
phases of scene 6). No going back once scene 6 starts.

## Scene 1 — Language

- Small window: language combobox + **Continue**.
- Default selection = OS UI language if supported, else English.
- The choice applies to the whole run (`SetThreadUILanguage`). No language
  switch later; restart the app to change it.

## Scene 2 — Message of the day

- Read-only multi-line text: the MOTD from the server (`GET /motd?lang=xx`,
  English fallback). Offline → built-in localized text saying the server is
  unreachable and only cached content is available.
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
  - exactly one match → dropdown shows that game (disabled);
  - several matches → dropdown enabled, listing only the matched games;
  - no match → dropdown back to (Detecting).
- **Warning modal** when no marker is found — after Browse returns and when
  Next is pressed, never per keystroke.
- **Next** enabled only when a game is detected/selected.
- **UNINSTALL** button enabled only when a game is detected/selected
  - Leads to a Backup restoration window, where previous backup packages can be selected

## Uninstall scene

- List of selectable previous backup packages
- **Restore** button available when a package is selected
- We switch to progress scene & install everything that is present in the restore package

## Scene 4 — Download

- One tree view with checkboxes, multi-level (see reference screenshot):

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

  Level 1 = `category`; middle levels = `tree_path` segments; leaves =
  packages. Only packages for the detected game are shown.
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
- Checked `pacenotes`-category packages are installed as-is (no choice).
- **Install** enabled when all three lists have a selection.

## Scene 6 — Installation

- Overall progress bar, phase label, current file with size
  ("Numeric 1to7.zip — 1.2 / 4.8 MB"), live log pane.
- Phases, in order:
  1. **Download** checked packages missing from the cache (sha256 verified);
  2. **Backup** files the install will overwrite and the installer does not
     already own (first touch only, under `backup\` in the game folder);
  3. **Install** the three configured packages + checked `pacenotes`
     packages; RBR additionally runs `write_rbr_config`.
- Phase order differs from the brief (backup before download): the backup
  needs each package's file list, which comes from its zip.
- Cancel allowed during phase 1 only; partial downloads are discarded.
- On error: stop, keep the log visible, go to scene 7 in failure state.

## Scene 7 — Summary

- Result: success / failure.
- What was done: game + folder, the three installed choices, `pacenotes`
  packages, number and total size of downloads, backup location.
- Log pane: collapsed on success, expanded on failure.
- **Finish** closes the app.

## Logging

Every scene appends to an in-memory log, also written to
`%LOCALAPPDATA%\PacenotesInstaller\install.log` (overwritten per run). The
log is visible on scenes 6 and 7; any error shows it.

## Implementation notes

- Wizard host: comctl32 **PropertySheet in wizard mode** (`PSH_WIZARD97` or
  `PSH_AEROWIZARD`) — Back/Next/Cancel and page switching come from the OS
  at ~0 bytes. Primary label via `PSM_SETNEXTTEXT` / `PSM_SETFINISHTEXT`,
  enablement via `PSM_SETWIZBUTTONS`. Fallback if it fights the design: one
  host dialog + child page dialogs swapped with `ShowWindow`.
- Tree: `SysTreeView32` + `TVS_CHECKBOXES`, partial state via
  `TVS_EX_PARTIALCHECKBOXES`; downloaded leaves bold (`TVIS_BOLD`) + suffix.
- All network work on the worker thread (I4 protocol); the UI thread only
  updates controls.
- Protocol additions needed: `PackageInfo.game`, `PackageInfo.tree_path`,
  server `GET /motd`.
