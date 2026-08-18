$root = "target/$env:RELEASE_TARGET/release/bundle"
$msi = @(Get-ChildItem "$root/msi/*.msi")
$nsis = @(Get-ChildItem "$root/nsis/*.exe")
if ($msi.Count -ne 1) { throw "Expected exactly one MSI, found $($msi.Count)" }
if ($nsis.Count -ne 1) { throw "Expected exactly one NSIS installer, found $($nsis.Count)" }
