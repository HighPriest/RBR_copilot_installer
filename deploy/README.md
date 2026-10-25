# Deploying the Pacenotes server

Written for: whoever administers the box that hosts the package server.

One Podman Quadlet builds the server straight from git, one Caddy site block
puts it on HTTPS, and one directory holds everything mutable. There is no
database to provision and no upload API: **publishing a package is copying a
zip into place.**

- `Containerfile` — multi-stage build of `pacenotes-server` (build context is
  the repository root).
- `entrypoint.sh` — regenerates `manifest.json`, then serves.
- `pacenotes.container` — the Quadlet unit. Builds from git on every start.
- `pacenotes.config.env` — all configuration. Copy and edit this.
- `Caddyfile.snippet` — the reverse-proxy site block.

---

## 1. One-time setup

### 1.1 Create the data directory

Everything the server can change lives in one place, so this is also the only
thing to back up:

```sh
mkdir -p ~/pacenotes/data/rbr/packages/public
mkdir -p ~/pacenotes/data/motd
# Every game needs its install schema, or NOTHING is published for it:
cp schemas/rbr.toml ~/pacenotes/data/rbr/schema.toml
```

Layout (see root `CLAUDE.md` for the reasoning — the folder tree IS the
installer's UI tree):

```
~/pacenotes/data/
├── manifest.json                  generated; never hand-edit
├── auth.db                        credentials; the one irreplaceable file
├── motd/
│   ├── en.txt                     message of the day, per language
│   └── pl.txt
├── reports/                       conflict reports from installers (S10)
└── <game>/                        rbr | dirtrally2 | assettocorsa | beamng
    ├── schema.toml                the game's install schema (see below)
    └── packages/
        ├── public/                visible to everyone
        │   ├── language/polish.zip
        │   ├── style/Descriptive.zip
        │   └── codriver/polish/male/KamilFilipek.zip
        └── <other-directory>/     visible only to granted access keys
```

Rules that will bite you if ignored:

- **`public` is a level of its own.** `rbr/packages/codriver/...` publishes
  `codriver` as an *access directory*, and every package's category shifts up
  a level. `manifest gen` warns about exactly this.
- Game ids and directory names are `[a-z0-9_-]+` — lowercase, no spaces.
- Package names may contain spaces (`style/Numeric Standard.zip` does), but a
  client has to percent-encode them. `manifest gen` warns so you can rename.
- `motd` and `reports` are reserved names at the top level: never games.

### 1.1a The install schema (one per game)

`<game>/schema.toml` lists the game's categories — the folders directly
under an access directory — whether each asks the user for exactly one pick
(`select = "one"`) or is optional (`"any"`), and where it installs
(`install_root`, a token such as `"game"`, never a path):

```toml
[[category]]
id = "codriver"
select = "one"
install_root = "game"
```

`manifest gen` is strict about it, because this is what tells an installer
whether it understands your catalogue:

- no `schema.toml` → **nothing** is published for that game (an `ERROR`);
- a schema that does not parse → nothing for that game (an `ERROR`);
- a package in a category the schema does not declare → that package is
  skipped (a warning). Adding a folder like `extras/` is a schema change.

The manifest carries each schema with the SHA-256 of the file's **exact
bytes**. An installer compares that with the schema it was built with and,
if they differ, refuses to DOWNLOAD for that game (reconfiguring and
uninstalling keep working). So:

- Copy schemas from the repository's `schemas/<game>.toml` with `cp` —
  never retype or reformat them. Even a changed comment or a CRLF line
  ending is a different schema to every installer.
- `manifest gen` warns when your copy differs from the one this server
  release was built with: that is the "installers will refuse" moment.
- **Changing a schema is a release, not an edit.** Ship an installer that
  accepts both the old and the new schema first; only then copy the new
  file onto the server. In the other order every installer in the wild
  stops downloading for that game until its users update.

### 1.2 Install the unit

```sh
cp deploy/pacenotes.container   ~/.config/containers/systemd/
cp deploy/pacenotes.config.env  ~/.config/containers/systemd/
```

Edit `~/.config/containers/systemd/pacenotes.config.env`. At minimum check
`GIT_REPO_URL` and `BRANCH_TO_BUILD`; leave `PACENOTES_LISTEN=0.0.0.0:8080`
alone (on `127.0.0.1` the container would be unreachable from Caddy).

Pulling over SSH means the host needs a key the git remote accepts — Test it before starting the unit:

```sh
git ls-remote "$(. ~/.config/containers/systemd/pacenotes.config.env; echo $GIT_REPO_URL)" >/dev/null && echo "git access OK"
```

### 1.3 Start it

```sh
systemctl --user daemon-reload
systemctl --user start pacenotes
journalctl --user -u pacenotes -f        # first build takes a few minutes
```

Check it from the host, without going through Caddy:

```sh
podman exec pacenotes curl -s localhost:8080/health          # -> ok
podman exec pacenotes curl -s localhost:8080/packages | head -c 200
```

### 1.4 Put it behind Caddy

Append `deploy/Caddyfile.snippet` to `~/caddy/caddy_etc/Caddyfile` (adjust
the hostname), then — if you want the internal `http://` block as well — add
the alias to `~/.config/containers/systemd/caddy.container`:

```
NetworkAlias=pacenotes.example.com
```

Reload (a Caddyfile edit alone needs no restart; the NetworkAlias does):

```sh
systemctl --user reload caddy          # Caddyfile only
systemctl --user restart caddy         # if you touched caddy.container
curl -s https://pacenotes.example.com/health
```

DNS must already point at this host, as for every other `*.example.com` site.
Caddy fetches the certificate itself.

---

## 2. Publishing packages

```sh
# 1. drop the zip in the right folder
cp KamilFilipek.zip ~/pacenotes/data/rbr/packages/public/codriver/polish/male/

# 2. publish
systemctl --user restart pacenotes
```

The restart regenerates `manifest.json` (that is what `entrypoint.sh` does),
so there is no separate publish step to forget. It also rebuilds the image
from git, which takes minutes; to publish without a rebuild:

```sh
podman exec pacenotes pacenotes-server manifest gen --data /data
```

The server picks the new manifest up on the next request — no restart needed
after that command.

**Read the warnings.** `manifest gen` prints one line per package and then
its complaints: mis-filed levels, a sidecar with no zip, a dependency that
does not exist, a name that needs URL-encoding. It is the only place those
mistakes are easy to see.

`ERROR:` lines are worse than warnings: that package was **withheld** (a
sidecar that does not parse, a zip outside an access directory). Everything
else still publishes, and the command exits **2** so a script cannot mistake
it for a clean run; on container start the entrypoint prints a banner for
it. Exit 1 means nothing was written and the previous manifest is still
being served.

### Optional metadata (sidecars)

Next to `<name>.zip`, an optional `<name>.toml`:

```toml
display_name = "Kamil Filipek"            # renames this package in the UI
description = "Calm, precise delivery"
version = "3.0.0"
exclusive_group = "pacenote-config"       # only one of a group installs
requires = ["public/language/polish"]     # "directory/name"
```

Nothing about *position* goes in here — the folders decide game, access
directory, category and tree. An unknown key is an error that withholds the
package, so a sidecar left over from an older layout (`category`,
`tree_path`, `game`) never publishes something wrong.

**The sidecar is the only source.** What is not in a sidecar is not in the
manifest: delete a key and it is gone at the next publish. With no
`version`, a package gets `sha-` plus the first 8 hex characters of its
sha256 — it changes exactly when the bytes do. The manifest is therefore
reproducible from the zips, sidecars and schemas alone.

Because removing metadata is now possible, `manifest gen` will not do it
silently: if a run would drop a value the current manifest carries, it
refuses, writes nothing and names each value. Re-run with
`--accept-metadata-loss` when the removal is intended.

**Upgrading from a server older than 2026-10-25** — do both, in this
order, BEFORE restarting the unit onto the new version:

1. Give every game its install schema. Without it the new server publishes
   nothing for that game:

   ```sh
   cp schemas/rbr.toml ~/pacenotes/data/rbr/schema.toml
   ```

2. Older servers carried metadata forward from the previous manifest, so
   yours may hold values that exist in no sidecar. Move them into sidecars
   and review what it wrote:

   ```sh
   podman exec pacenotes pacenotes-server manifest migrate-to-sidecars --data /data
   ```

   It only adds keys a sidecar lacks (never overwrites), never pins the old
   `0.0.0` default, and is safe to re-run.

Forgetting either is not an outage: `manifest gen` refuses to write a
manifest that would take a live game off the server (missing schema) or
drop metadata, so the old catalogue keeps being served and the container log
says why. `--accept-unpublish` / `--accept-metadata-loss` override when that
is really what you want.

### Message of the day

```sh
echo "Server is up. 24 packages available." > ~/pacenotes/data/motd/en.txt
```

Served at `GET /motd?lang=en`, with `en` as the fallback for any language
that has no file. No file at all is a 404, which the installer treats exactly
like being offline — so skipping this costs the user nothing.

### Conflict reports

When two packages a user selected write the same file, the installer lets
the later one win, carries on, and tells the server
(`POST /report/conflict`). Only you can fix such a catalogue — give the
packages an `exclusive_group`, or move the shared file into one of them —
so these reports exist to reach you:

- They land in `~/pacenotes/data/reports/conflicts-YYYY-MM-DD.jsonl`, one
  JSON line each.
- `manifest gen` summarises the last 30 days as warnings, one per pair of
  packages, so you see them at the moment you publish.

**What a report contains**, if a user asks: the game, the installer's
version, and for each shared file the file's path inside the game folder and
the packages that wrote it. Nothing else — no access key, no computer name,
no path on the user's machine, and the server does not store the sender's
IP address (it keeps it in memory only, to rate-limit).

The endpoint takes no key, so it is limited per client (10 reports an hour)
and overall (600 an hour). Behind Caddy every request arrives from Caddy's
address, so the server must be told to believe Caddy's `X-Forwarded-For`
header — and only Caddy's:

```sh
podman network inspect internal --format '{{range .Subnets}}{{.Subnet}}{{end}}'
# -> 10.0.0.0/24 ; then in pacenotes.config.env:
PACENOTES_TRUSTED_PROXIES=10.0.0.0/24
```

Leave it empty and nothing breaks: the header is ignored, and all clients
simply share one bucket.

---

## 3. Access keys

`public/` needs no key. Every other directory needs a credential that was
granted it. The server stores only `sha256(key)`, so a key is shown **once**,
at mint time, and cannot be recovered afterwards.

```sh
# a directory for paid content, then a key that may see it
mkdir -p ~/pacenotes/data/rbr/packages/premium/codriver
podman exec pacenotes pacenotes-server token mint \
    --label "Jan (patron)" --grant premium --expires-days 365

podman exec pacenotes pacenotes-server token list
podman exec pacenotes pacenotes-server token grant   <token-or-hash> beta
podman exec pacenotes pacenotes-server token ungrant <token-or-hash> beta
podman exec pacenotes pacenotes-server token revoke  <token-or-hash>
```

- `--grant` is repeatable; a key with no grants is valid and sees only
  `public`.
- `token list` prints a short hash, which is what the other commands accept
  in place of the key itself.
- `public` cannot be granted (it is already universal) and the CLI refuses.
- `ungrant` and `revoke` take effect on the next request: grants are read per
  request, never cached.

`GET /packages` never returns 401/403. A caller with a bad, expired or
revoked key silently sees the public listing, so nobody can discover
directory names by probing. Downloads are where 401/403 appear.

---

## 4. Testing the installer against it

The installer only needs a base URL:

- `https://pacenotes.example.com` once Caddy is up, or
- `http://<host-ip>:8080` if you add `PublishPort=8080:8080` to
  `pacenotes.container` for a quick local test. Do not leave it published:
  the server speaks plain HTTP on purpose and expects Caddy to do TLS.

Useful checks from a workstation:

```sh
curl -s https://pacenotes.example.com/health
curl -s https://pacenotes.example.com/packages?game=rbr | python3 -m json.tool | head
curl -s "https://pacenotes.example.com/motd?lang=en"
curl -sI https://pacenotes.example.com/download/rbr/public/language/polish
curl -sI -H "Authorization: Bearer pcn_…" \
     https://pacenotes.example.com/download/rbr/premium/codriver/polish/male/Someone
```

Note the download URL shape: `/download/{game}/{directory}/{name}`, where
`{name}` is the package's path under the access directory and must be
percent-encoded (`style/Numeric%20Standard`).

---

## 5. Operating

| Task | Command |
|---|---|
| Deploy the latest commit | `systemctl --user restart pacenotes` |
| Logs | `journalctl --user -u pacenotes -f` |
| Health | `podman healthcheck run pacenotes` |
| Publish without rebuilding | `podman exec pacenotes pacenotes-server manifest gen --data /data` |
| Back up | `~/pacenotes/data` — `auth.db` above all, and the packages + sidecars (your content; nothing else has a copy). `manifest.json` is regenerated from them |

`Restart=on-failure` brings a crashed server back, but a failed *build* stops
the unit rather than looping: the build runs in `ExecStartPre`.

### Troubleshooting

**`manifest gen` warns that a category is at the top level.** The
access-directory level is missing. Move the category folders under `public/`
(or another directory) and restart.

**A download 404s that should work.** Either the manifest is stale (run
`manifest gen`) or the name needs percent-encoding. `{game}`, `{directory}`
and `{name}` are manifest lookup keys, never paths, so anything not listed —
including every traversal attempt — is a 404 by design.

**A download 401s with a key that should work.** `token list` shows whether
it is revoked or expired, and which directories it was granted. A key with no
grant for that directory gets 403, not 401 — 401 means the key itself was
rejected.

**The build fails with layers "vanishing"**
(`identifier is not a container`, `layer not known`, or rustc reporting
`No such file or directory` under `target/`). Another podman build on this
host ran `podman image prune -f --external`, which deletes *concurrent*
builds' intermediate containers. The quadlets here (including
`pacenotes.container`) do exactly that in `ExecStartPre`, so do not build two of
them at once — and if one of those units is stuck in a restart loop, fix that
first. To build regardless, use a separate storage root:

```sh
podman --root ~/.cache/pacenotes-build/storage \
       --runroot ~/.cache/pacenotes-build/run \
       build --network host -f deploy/Containerfile -t localhost/pacenotes-server:test .
```

Clean that scratch storage up with `podman unshare`, not plain `rm` — the
layers are owned by subuids, so `rm -rf` fails with "Permission denied":

```sh
podman unshare rm -rf ~/.cache/pacenotes-build
```

**The build runs out of memory.** This host has 3 GB. The Containerfile
pins `CARGO_BUILD_JOBS=1` for that reason; OOM shows up as the same
"vanishing layer" symptom rather than an honest error. On a bigger builder:
`--build-arg CARGO_BUILD_JOBS=4`.
