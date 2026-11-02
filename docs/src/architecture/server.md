# The server

## One download request

![S1: how one download request is answered](diagrams/activity-server.svg)

The whole access model is one table:

| Situation | Answer |
|---|---|
| the `public` directory | the package, or 404 |
| a protected directory, no live key | **401**, whether or not the package exists |
| a protected directory, a live key without the grant | **404**, the same as "no such package" |
| a protected directory, a live key with the grant | the package, or 404 |

There is no 403: "exists, but not for you" is the enumeration signal this
avoids. `/packages` is the exception that proves the rule: it never answers
4xx, filters silently to the public and granted directories, and falls back
to the public listing when the database is down. A key is validated with
`/whoami`, not with `/packages`.

The URL parts (`game`, `directory`, `name`) are **manifest lookup keys**,
never path fragments: only a manifest hit yields
`data/<game>/packages/<directory>/<name>.zip`. That is what prevents
traversal structurally, and what makes the `{*name}` wildcard safe.

Two smaller decisions the diagram shows: an `Authorization` value that
cannot be a minted key is turned away before any query, so garbage stays
cheap; and a file whose size disagrees with the manifest is refused with 503
rather than streamed — the client would fail its own sha256 check anyway.

## Publishing

![S2: publishing with manifest gen](diagrams/activity-manifest-gen.svg)

Publishing is copying a zip into place and running one command. The command
is strict on purpose:

- no `schema.toml` → the game publishes **nothing** (an error, not a warning);
- a package in a category the schema does not declare → skipped, with a warning;
- a sidecar with an unknown key → **that package** is withheld, the rest
  still publishes, and the run exits 2;
- metadata the current manifest carries would disappear → refused, exit 1,
  nothing written, unless `--accept-metadata-loss`;
- a game that is live now would vanish because its schema is missing →
  refused, exit 1, unless `--accept-unpublish`.

The manifest is a **pure function** of zips, sidecars and schemas: nothing is
inherited from the previous one, so deleting a sidecar key removes it at the
next publish. The two guards exist because that purity is also what lets an
accidental upgrade empty a live catalogue.

`manifest gen` also reads the last 30 days of conflict reports and turns
them into per-pair warnings: a user's bad day becomes something the operator
can fix in the catalogue.
