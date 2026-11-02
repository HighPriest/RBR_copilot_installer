# Binary size report

The installer has a hard size gate (see
[Installer requirements](installer/requirements.md), "The size budget").
This is the last `cargo bloat` measurement saved to `docs/bloat.txt`: where
the bytes of the release build go, by crate.

```text
{{#include ../bloat.txt}}
```
