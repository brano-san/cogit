# Draws the NSIS sidebar (Welcome/Finish) and header bitmaps from the app icon. Twice the
# MUI2 size (164x314, 150x57): MUI stretches to the control, so 100-200% all stay sharp.
# Run again after the icon changes: powershell -File src-tauri/windows/make-bitmaps.ps1
Add-Type -AssemblyName System.Drawing
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$icon = [System.Drawing.Image]::FromFile((Join-Path $here "..\icons\128x128@2x.png"))

function Save-Bmp($bmp, $name) {
    $bmp.Save((Join-Path $here $name), [System.Drawing.Imaging.ImageFormat]::Bmp)
    $bmp.Dispose()
}

# Sidebar: the app's dark background (bg.app), the icon and the name.
$side = New-Object System.Drawing.Bitmap 328, 628, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [System.Drawing.Graphics]::FromImage($side)
$g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"; $g.TextRenderingHint = "AntiAliasGridFit"
$top = [System.Drawing.ColorTranslator]::FromHtml("#1d2330")
$bottom = [System.Drawing.ColorTranslator]::FromHtml("#141619")
$rect = New-Object System.Drawing.Rectangle 0, 0, 328, 628
$g.FillRectangle((New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, $top, $bottom, 90), $rect)
$g.DrawImage($icon, 84, 150, 160, 160)
$font = New-Object System.Drawing.Font "Segoe UI Semibold", 30, ([System.Drawing.GraphicsUnit]::Pixel)
$format = New-Object System.Drawing.StringFormat; $format.Alignment = "Center"
$g.DrawString("Cogit", $font, (New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml("#e6edf3"))), (New-Object System.Drawing.RectangleF 0, 330, 328, 50), $format)
$g.Dispose(); Save-Bmp $side "nsis-sidebar.bmp"

# Header: sits on the white page header, so white with the icon at the right edge.
$head = New-Object System.Drawing.Bitmap 300, 114, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [System.Drawing.Graphics]::FromImage($head)
$g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"
$g.Clear([System.Drawing.Color]::White)
$g.DrawImage($icon, 196, 13, 88, 88)
$g.Dispose(); Save-Bmp $head "nsis-header.bmp"
# MSI (WixUI): the banner over every inner dialog (493x58) and the Welcome/Exit background
# (493x312), where the text sits on the white right part. Twice the size, as above.
$banner = New-Object System.Drawing.Bitmap 986, 116, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [System.Drawing.Graphics]::FromImage($banner)
$g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"
$g.Clear([System.Drawing.Color]::White)
$g.DrawImage($icon, 870, 14, 88, 88)
$g.Dispose(); Save-Bmp $banner "wix-banner.bmp"

$dialog = New-Object System.Drawing.Bitmap 986, 624, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
$g = [System.Drawing.Graphics]::FromImage($dialog)
$g.SmoothingMode = "AntiAlias"; $g.InterpolationMode = "HighQualityBicubic"; $g.TextRenderingHint = "AntiAliasGridFit"
$g.Clear([System.Drawing.Color]::White)
$panel = New-Object System.Drawing.Rectangle 0, 0, 328, 624
$g.FillRectangle((New-Object System.Drawing.Drawing2D.LinearGradientBrush $panel, $top, $bottom, 90), $panel)
$g.DrawImage($icon, 84, 150, 160, 160)
$g.DrawString("Cogit", $font, (New-Object System.Drawing.SolidBrush ([System.Drawing.ColorTranslator]::FromHtml("#e6edf3"))), (New-Object System.Drawing.RectangleF 0, 330, 328, 50), $format)
$g.Dispose(); Save-Bmp $dialog "wix-dialog.bmp"
$icon.Dispose()
