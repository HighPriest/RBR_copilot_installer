<#
.SYNOPSIS
    Rebuild rbr.ico from rbr-source.png. Run it when the artwork changes.

.DESCRIPTION
    The icon is the single largest thing in the installer binary, so it is
    built deliberately rather than exported from a drawing tool.

    What it used to be: ONE 256x256 PNG frame, 85 kB — more than serde and
    the whole install engine put together, in a binary with a 600 kB gate.
    Windows then downsampled that frame to 16x16 for the title bar, which
    is both the most expensive and the least sharp way to get there.

    What it is now, and why:

      16, 32, 48  as uncompressed BMP.  These are the sizes that actually
                  appear (title bar, alt-tab, Explorer's medium icons).
                  BMP rather than PNG because a PNG-compressed small frame
                  is accepted by modern Windows but not by every shell
                  path that has ever rendered an icon, and 15 kB buys
                  certainty.
      128         as PNG. Covers Explorer's large (96) and extra-large
                  (256) views: 96 is a clean downscale and 256 upscales
                  acceptably for a tile nobody stares at.
      256         DROPPED. It was 85 kB of the 100 kB the old icon cost,
                  for a size seen in one Explorer view.

    Net: about 45 kB instead of 85 kB, and sharper at the sizes that are
    actually used, because each small frame is resampled from the source
    rather than from a 256-pixel image at display time.

.EXAMPLE
    pwsh -File installer/res/Build-Icon.ps1
#>
[CmdletBinding()]
param(
    [string]$Source = (Join-Path $PSScriptRoot 'rbr-source.png'),
    [string]$Out = (Join-Path (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent) 'rbr.ico')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

if (-not (Test-Path $Source)) { Write-Error "No source image at $Source"; exit 1 }

# Sizes, and how each is stored. See the description for why.
$asBmp = 16, 32, 48
$asPng = , 128

function Resize-Frame {
    param([System.Drawing.Image]$Image, [int]$Size)
    $bmp = New-Object System.Drawing.Bitmap $Size, $Size,
        ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.PixelOffsetMode = 'HighQuality'
    $g.SmoothingMode = 'HighQuality'
    $g.CompositingQuality = 'HighQuality'
    $g.CompositingMode = 'SourceCopy'   # keep the source alpha, do not blend
    $g.DrawImage($Image, 0, 0, $Size, $Size)
    $g.Dispose()
    return $bmp
}

<#  A 32bpp DIB as an .ico frame: BITMAPINFOHEADER, the pixels bottom-up in
    BGRA, then an AND mask. The mask is ignored for 32bpp images but the
    format still requires it, and the header still has to claim DOUBLE the
    real height — the one part of this format everybody gets wrong once. #>
function ConvertTo-IconDib {
    param([System.Drawing.Bitmap]$Bitmap)
    $n = $Bitmap.Width
    $out = New-Object IO.MemoryStream
    $w = New-Object IO.BinaryWriter $out
    $w.Write([uint32]40); $w.Write([int32]$n); $w.Write([int32]($n * 2))
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]0); $w.Write([uint32]0)          # BI_RGB, size ignored
    $w.Write([int32]0); $w.Write([int32]0)            # pixels per metre
    $w.Write([uint32]0); $w.Write([uint32]0)          # palette

    for ($y = $n - 1; $y -ge 0; $y--) {
        for ($x = 0; $x -lt $n; $x++) {
            $c = $Bitmap.GetPixel($x, $y)
            $w.Write([byte]$c.B); $w.Write([byte]$c.G)
            $w.Write([byte]$c.R); $w.Write([byte]$c.A)
        }
    }
    # AND mask: one bit per pixel, rows padded to four bytes, all zero.
    $maskRow = [math]::Ceiling($n / 8.0)
    if ($maskRow % 4 -ne 0) { $maskRow += 4 - ($maskRow % 4) }
    $w.Write((New-Object byte[] ($maskRow * $n)))
    $w.Flush()
    $bytes = $out.ToArray()
    $w.Dispose(); $out.Dispose()
    # The leading comma matters: PowerShell unrolls an array returned from
    # a function, and the caller would get 1128 separate bytes in an
    # Object[] whose .Length still reads 1128 — an icon that looks right
    # until you open it.
    return , $bytes
}

$src = [System.Drawing.Image]::FromFile((Resolve-Path $Source))
try {
    $frames = @()
    foreach ($n in ($asBmp + $asPng | Sort-Object)) {
        $bmp = Resize-Frame -Image $src -Size $n
        if ($asBmp -contains $n) {
            $data = ConvertTo-IconDib -Bitmap $bmp
        } else {
            $ms = New-Object IO.MemoryStream
            $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
            $data = $ms.ToArray(); $ms.Dispose()
        }
        $bmp.Dispose()
        $frames += [pscustomobject]@{ Size = $n; Data = $data }
        "  {0,3}px  {1,7:N0} bytes  {2}" -f $n, $data.Length,
            $(if ($asBmp -contains $n) { 'BMP' } else { 'PNG' }) | Write-Host
    }

    # Not $out: PowerShell variable names are case-insensitive, so that
    # would be the [string]$Out parameter and the stream would be stringified.
    $ico = New-Object IO.MemoryStream
    $w = New-Object IO.BinaryWriter $ico
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$frames.Count)
    # Entries come first, so every offset is past all of them.
    $offset = 6 + 16 * $frames.Count
    foreach ($f in $frames) {
        $w.Write([byte]$(if ($f.Size -ge 256) { 0 } else { $f.Size }))
        $w.Write([byte]$(if ($f.Size -ge 256) { 0 } else { $f.Size }))
        $w.Write([byte]0); $w.Write([byte]0)          # palette, reserved
        $w.Write([uint16]1); $w.Write([uint16]32)     # planes, bpp
        $w.Write([uint32]$f.Data.Length); $w.Write([uint32]$offset)
        $offset += $f.Data.Length
    }
    foreach ($f in $frames) { $w.Write([byte[]]$f.Data, 0, $f.Data.Length) }
    $w.Flush()
    [IO.File]::WriteAllBytes($Out, $ico.ToArray())
    $w.Dispose(); $ico.Dispose()
} finally {
    $src.Dispose()
}

$size = (Get-Item $Out).Length
Write-Host ("wrote {0} ({1:N0} bytes, {2} frames)" -f $Out, $size, $frames.Count) -ForegroundColor Green
