# One installer run

![A1: one run of the installer, three routes](diagrams/activity-installer.svg)

## The pages

| Scene | Page | What it decides | Source |
|---|---|---|---|
| 1 | `IDD_S1_LANGUAGE` | the UI language; defaults to Windows' own | `wizard/scene/language.rs` |
| 2 | `IDD_S2_MOTD` | the message of the day, and an optional access key | `wizard/scene/message_of_the_day.rs`, `api/` |
| 3 | `IDD_S3_FOLDER` | the game folder, which game it is (its marker `.exe`), and the route | `wizard/scene/game_folder.rs`, `game_folder_routes.rs`, `gamedir/` |
| 4 | `IDD_S4_DOWNLOAD` | what to have on this computer: catalogue, cache and ledger as one tree | `wizard/scene/download.rs`, `tree/` |
| 4b | `IDD_S4B_FETCH` | the transfers: cache only, nothing is installed | `wizard/scene/fetching.rs`, `package_download.rs` |
| 5 | `IDD_S5_CONFIG` | one pick per choice category, plus the optional extras | `wizard/scene/configuration.rs`, `choices/`, `schema.rs` |
| 6 | `IDD_S6_INSTALL` | the run itself | `wizard/scene/installation.rs`, `engine/` |
| 7 | `IDD_S7_SUMMARY` | what changed, and where the log is | `wizard/scene/summary.rs` |
| — | `IDD_RESTORE` | restore points: preview, then carry out | `wizard/scene/restore.rs`, `restore/` |

**Three routes**, which part at scene 3 and never fork again:

| Route | Path | Reaches the network? |
|---|---|---|
| Download / Update | 4 → 4b → 5 → 6 → 7 | yes: the catalogue and the downloads |
| Configure / Install | 5 → 6 → 7 | no |
| Restore / Uninstall | restore → 6 → 7 | no |

That split is why there is no separate "offline mode": Configure *is* the
Download route with the two transfer pages left out.

Decisions on the diagram worth reading twice:

- **Scene 4's ticks are what the user wants to HAVE; scene 5's picks are
  what to USE.** So downloading happens before the questions, and the
  install page works from the cache alone.
- **Scene 5 opens only when every question has something to pick from**: a
  ticked package of each choice category whose payload is in the cache.
  Configure/Install checks on scene 3 and stays there; Download checks after
  4b and goes back to scene 4, with what never arrived unticked.
- **An unknown schema hash blocks downloading, and only for that game.** An
  installer that is behind may still reconfigure, uninstall and restore what
  is already on the machine: taking a working setup away from someone for
  not updating would be the worst possible answer.

Cancel is the sheet's button, not a route: honoured on every page before
scene 6 and disabled there, because from then on every phase writes to the
game folder and the engine has no stop between files.

## Scene 6: the engine

![A2: the install engine](diagrams/activity-engine.svg)

The order is the safety argument, not a style:

1. **Plan** what to uninstall (dropped, superseded by a pick or an
   `exclusive_group`, half-written, or changed) and what to install.
   Dependencies are checked against the **selection**, not the ledger:
   anything installed and unselected is removed a few lines later.
2. **Pre-flight** everything that can fail — payload hash, write probe, free
   space, the game running from this folder, every file list — while the
   folder is untouched.
3. **Open the restore point**, holding the ledger as it is now.
4. **Uninstall first, always.** Several packages ship the same paths, so
   installing before uninstalling would delete what was just written.
5. Per package, with its zip's directory read once: classify each file's
   `prior`, write the ledger entry as `installing`, extract through a temp
   file and a rename, then mark it `installed` with each file's hash. A file
   that already holds the incoming bytes is hashed and left as it is.
6. Run the game's **own** configuration step, then adopt the hashes of the
   files it edited — otherwise the next uninstall would read our own edit as
   somebody else's and leave the file behind.
7. Report overlapping files to the server; a failed report is a log line.

The engine is pure `std` and has to stay testable off Windows, so the two
facts it cannot measure — free space, and whether the game is running from
this folder — are passed in by the UI thread as `HostFacts`.
