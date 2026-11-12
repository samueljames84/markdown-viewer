param(
    [string]$Exe = (Join-Path $PSScriptRoot '..\target\x86_64-pc-windows-msvc\release\mdviewer.exe'),
    [string]$Document = (Join-Path $PSScriptRoot '..\tests\fixtures\plain.md'),
    [int]$Samples = 3,
    [int]$SettleSeconds = 5,
    [int]$IdleSeconds = 10
)
$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path $Exe).Path
$documentPath = (Resolve-Path $Document).Path
$root = Join-Path ([System.IO.Path]::GetTempPath()) ('mdviewer-measure-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$originalRoaming = $env:APPDATA
$originalLocal = $env:LOCALAPPDATA
$measurements = @()
$app = $null
try {
    foreach ($sample in 1..$Samples) {
        $sampleDir = Join-Path $root "sample-$sample"
        $env:APPDATA = Join-Path $sampleDir 'roaming'
        $env:LOCALAPPDATA = Join-Path $sampleDir 'local'
        $settingsDir = Join-Path $env:APPDATA 'MdViewer'
        New-Item -ItemType Directory -Force $settingsDir,$env:LOCALAPPDATA | Out-Null
        $paths = @($documentPath, (Join-Path $PSScriptRoot '..\tests\fixtures\plain.md'), (Join-Path $PSScriptRoot '..\tests\fixtures\features.md')) | ForEach-Object { (Resolve-Path $_).Path } | Select-Object -Unique
        $tabs = @($paths | ForEach-Object { @{ path = @($_.ToCharArray() | ForEach-Object { [int]$_ }); scroll = 0 } })
        @{ version = 1; tabs = $tabs; active = 0; theme = 'light'; colors = @('#ffffff','#2864b4'); zoom = 100; window = @{x=100;y=100;width=1100;height=760;maximized=$false;dpi_scale=1} } | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $settingsDir 'settings.json') -Encoding utf8NoBOM
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $app = Start-Process -FilePath $exePath -PassThru
        $log = Join-Path $settingsDir 'mdviewer.log'
        $text = ''
        while ($clock.Elapsed.TotalSeconds -lt 30) {
            if ($app.HasExited) { throw "Viewer exited before rendering (code $($app.ExitCode))" }
            if (Test-Path $log) {
                $text = Get-Content $log -Raw
                if ($text -match 'METRIC active-content (\d+) ms') { break }
            }
            Start-Sleep -Milliseconds 20
        }
        if ($text -notmatch 'METRIC active-content (\d+) ms') { throw 'Timed out waiting for active document' }
        $contentMs = [int]$Matches[1]
        $null = $text -match 'METRIC shell (\d+) ms'
        $shellMs = [int]$Matches[1]
        Start-Sleep -Seconds $SettleSeconds
        $all = Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId,Name
        $ids = [Collections.Generic.HashSet[int]]::new()
        $null = $ids.Add($app.Id)
        do {
            $count = $ids.Count
            foreach ($process in $all) { if ($ids.Contains([int]$process.ParentProcessId)) { $null = $ids.Add([int]$process.ProcessId) } }
        } while ($count -ne $ids.Count)
        $processes = @($ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        $hostProcess = $processes | Where-Object Id -EQ $app.Id
        $cpuStart = ($processes | Measure-Object CPU -Sum).Sum
        $cpuClock = [Diagnostics.Stopwatch]::StartNew()
        Start-Sleep -Seconds $IdleSeconds
        $processes = @($ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue })
        $cpuEnd = ($processes | Measure-Object CPU -Sum).Sum
        $cpuPercent = ($cpuEnd - $cpuStart) / $cpuClock.Elapsed.TotalSeconds * 100
        $measurements += [pscustomobject]@{
            sample = $sample; restoredTabs = $tabs.Count
            shellMs = $shellMs; activeContentMs = $contentMs
            hostWorkingSetMiB = [math]::Round($hostProcess.WorkingSet64 / 1MB, 2)
            hostPrivateMiB = [math]::Round($hostProcess.PrivateMemorySize64 / 1MB, 2)
            treeWorkingSetMiB = [math]::Round(($processes | Measure-Object WorkingSet64 -Sum).Sum / 1MB, 2)
            treePrivateMiB = [math]::Round(($processes | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB, 2)
            treeIdleOneCorePercent = [math]::Round($cpuPercent, 3)
            processes = $processes.Count
        }
        $requested = $app.CloseMainWindow()
        if (!$requested -or !$app.WaitForExit(15000)) {
            Write-Host "CloseMainWindow returned: $requested; HWND: $($app.MainWindowHandle)"
            Write-Host (Get-Content $log -Raw)
            Stop-Process -Id $app.Id
            throw 'Viewer did not exit cleanly'
        }
        $app = $null
        foreach ($owned in $processes) {
            if (!$owned.HasExited -and !$owned.WaitForExit(3000)) {
                $current = Get-Process -Id $owned.Id -ErrorAction SilentlyContinue
                if ($current -and $current.StartTime -eq $owned.StartTime) { Stop-Process -Id $owned.Id }
            }
        }
    }
    [pscustomobject]@{
        executable = $exePath; bytes = (Get-Item $exePath).Length
        MiB = [math]::Round((Get-Item $exePath).Length / 1MB, 3)
        document = $documentPath; documentBytes = (Get-Item $documentPath).Length
        settleSeconds = $SettleSeconds; idleIntervalSeconds = $IdleSeconds
        windows = [Environment]::OSVersion.VersionString
        processor = (Get-CimInstance Win32_Processor -Property Name).Name
        note = 'Fresh per-sample WebView2 profiles; filesystem/runtime may be warm. Startup markers start at Rust run(), not process creation. Working sets may double-count shared pages. CPU percentage uses one logical core = 100%.'
        samples = $measurements
    } | ConvertTo-Json -Depth 6
} finally {
    if ($app -and !$app.HasExited) { Stop-Process -Id $app.Id }
    $env:APPDATA = $originalRoaming; $env:LOCALAPPDATA = $originalLocal
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        try { Remove-Item -LiteralPath $root -Recurse -Force; break }
        catch [System.IO.IOException], [System.UnauthorizedAccessException] {
            if ($attempt -eq 9) { throw }
            Start-Sleep -Milliseconds 500
        }
    }
}
