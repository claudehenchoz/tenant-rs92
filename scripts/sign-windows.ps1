# Authenticode-signs the Windows binaries in a folder (default: target\bundled).
# Requires WINDOWS_CERT_PFX (base64) and WINDOWS_CERT_PASSWORD.
param([string]$Dir = "target\bundled")
$ErrorActionPreference = "Stop"
[IO.File]::WriteAllBytes("cert.pfx", [Convert]::FromBase64String($env:WINDOWS_CERT_PFX))
$signtool = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\signtool.exe" | Select-Object -Last 1
Get-ChildItem $Dir -Recurse -Include *.vst3, *.clap, *.exe -File | ForEach-Object {
    & $signtool.FullName sign /f cert.pfx /p $env:WINDOWS_CERT_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $_.FullName
    if ($LASTEXITCODE -ne 0) { throw "signtool failed on $($_.FullName)" }
}
Remove-Item cert.pfx
