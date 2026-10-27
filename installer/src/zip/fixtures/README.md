# src/zip/fixtures

Binary inputs for the Rust tests. Two archives, used by `src/zip/tests.rs`.

## overlay-deflate.zip / overlay-stored.zip

Both hold the same thing — a miniature game-root overlay, which is the only
shape a real package ever has:

```text
Plugins/Pacenote/                                   (directory entry)
Plugins/Pacenote/config/                            (directory entry)
Plugins/Pacenote/sounds/                            (directory entry)
Plugins/Pacenote/config/pacenote/Rbr.ini            24 bytes, CRLF
Plugins/Pacenote/sounds/polish/male/Kamil/001.ogg   75 bytes
Plugins/Pacenote/sounds/polish/                     (directory entry)
Plugins/Pacenote/sounds/polish/male/                (directory entry)
aa/evil.txt                                         9 bytes
```

Three properties matter, and all three are why these are real archives
rather than bytes this repository wrote:

- **Not every directory is an entry.** `Plugins/` has no entry, and neither
  does the directory holding a nested file. An extractor that only created
  the directories it was told about would fail on the first real package.
- **One is deflated, one is stored.** `-CompressionLevel NoCompression`
  gives method 0 throughout; the default gives method 8. Both occur in the
  wild and both must read identically.
- **`aa/evil.txt` is exactly as long as `../evil.txt`.** The zip-slip test
  renames it in place, in both the local header and the central directory,
  which turns a valid archive into a hostile one without moving a single
  offset — and without this project owning a zip writer.

Produced with PowerShell's `Compress-Archive`, i.e. .NET's own `ZipArchive`:
the library that wrote every package the legacy installer ever read, and the
one `deploy/README.md` tells an operator to use. Reading back an archive we
had also written would only prove our two halves agree with each other.

To regenerate (from `installer/`):

```powershell
$src = "$env:TEMP\zipsrc"
New-Item -ItemType Directory -Force "$src\Plugins\Pacenote\config\pacenote",
    "$src\Plugins\Pacenote\sounds\polish\male\Kamil", "$src\aa" | Out-Null
Set-Content -NoNewline "$src\Plugins\Pacenote\config\pacenote\Rbr.ini" "[PACENOTE]`r`nname=Kamil`r`n"
Set-Content -NoNewline "$src\Plugins\Pacenote\sounds\polish\male\Kamil\001.ogg" `
    ("OggS`0fake audio payload, repeated: " + ('x' * 40))
Set-Content -NoNewline "$src\aa\evil.txt" "harmless`n"

Compress-Archive "$src\Plugins","$src\aa" src\zip\fixtures\overlay-deflate.zip -Force
Compress-Archive "$src\Plugins","$src\aa" src\zip\fixtures\overlay-stored.zip `
    -CompressionLevel NoCompression -Force
```

The byte counts in `src/zip/tests.rs` come from the files above, so if you
change the content, change the assertions with it.

## What these fixtures cannot tell you

That the reader copes with the packages that actually exist. For that there
is a MANUAL CHECK in `src/zip/tests.rs` which walks a directory of real ones:

```powershell
$env:PACENOTES_ZIP_CORPUS = "D:\Games\RichardBurnsRally\.pacenotes\cache"
cargo test --target x86_64-pc-windows-msvc --lib every_real_package -- --ignored --nocapture
```

Any directory of real package zips will do. A game folder the installer has
downloaded into is the easy one, since that is where its cache lives.
