$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    $metadata = cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'cargo metadata failed' }
    $parts = [Collections.Generic.List[string]]::new()
    $parts.Add("MdViewer third-party notices`n`nJavaScript/font package licenses (including development tools):")
    $parts.Add([IO.File]::ReadAllText((Join-Path $root 'frontend\vendor\LICENSES.txt')))
    $parts.Add("`nRust crate licenses (including build and test dependencies):")
    foreach ($package in ($metadata.packages | Where-Object source | Sort-Object name,version)) {
        $parts.Add("`n===== $($package.name) $($package.version) [$($package.license)] =====")
        $directory = Split-Path $package.manifest_path -Parent
        $files = Get-ChildItem -LiteralPath $directory -File | Where-Object Name -Match '^(license|licence|copying|notice)([-._]|$)'
        foreach ($file in $files) { $parts.Add($file.Name + "`n" + [IO.File]::ReadAllText($file.FullName)) }
    }
    [IO.File]::WriteAllText((Join-Path $root 'THIRD_PARTY_NOTICES.txt'), ($parts -join "`n"), [Text.UTF8Encoding]::new($false))
} finally { Pop-Location }
