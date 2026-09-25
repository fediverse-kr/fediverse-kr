# Actual offline command boundary checks. No source rows or remote requests.
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$binary = Join-Path $workspace 'target/debug/fediversekr2.exe'
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw 'Build the server binary first; this check does not build or start the website.'
}
$cases = @(
    @{ Arguments = @('--legacy-assets'); Message = 'Asset import requires' },
    @{ Arguments = @('--legacy-assets', '--apply'); Message = 'Asset import requires' },
    @{ Arguments = @('--legacy-activate'); Message = 'Activation requires' },
    @{ Arguments = @('--legacy-activate', '--apply'); Message = 'Activation requires' },
    @{ Arguments = @('--legacy-activate', '--apply'); Message = 'Activation requires'; MissingAck = $true }
)
$checks = 0
foreach ($case in $cases) {
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $binary
    $info.WorkingDirectory = $workspace
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $case.Arguments) { $info.ArgumentList.Add($argument) }
    foreach ($name in @('FEDKR_IMPORT_SOURCE_URL', 'FEDKR_IMPORT_TARGET_URL', 'FEDKR_ASSET_SOURCE_DIR', 'FEDKR_ASSET_BUNDLE_DIR', 'FEDKR_ACTIVATION_TARGET_URL', 'FEDKR_ACTIVATION_ACK', 'FEDKR_PUBLIC_ORIGIN')) {
        [void]$info.Environment.Remove($name)
    }
    if ($case.MissingAck) {
        $info.Environment['FEDKR_IMPORT_SOURCE_URL']='postgres://fixture@127.0.0.1:65535/fedkr_snapshot_fixture'
        $info.Environment['FEDKR_ACTIVATION_TARGET_URL']='postgres://fixture@127.0.0.1:65535/fedkr_live_fixture'
        $info.Environment['FEDKR_PUBLIC_ORIGIN']='https://cutover.example.org'
    }
    # Would fail runtime initialization if the offline branch fell through.
    $info.Environment['FEDKR_DATABASE_URL'] = 'not-a-database-url'
    $process = [System.Diagnostics.Process]::Start($info)
    try {
        $out = $process.StandardOutput.ReadToEndAsync()
        $err = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(15000)) {
            # This exact process was started above for this test; no other app.
            $process.Kill($true)
            $process.WaitForExit()
            throw 'Offline command did not exit within its test deadline.'
        }
        $stdout = $out.GetAwaiter().GetResult()
        $stderr = $err.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 1 -or $stdout.Trim().Length -ne 0 -or -not $stderr.Contains($case.Message)) {
            throw 'Offline command did not return its redacted configuration error.'
        }
        if ($stderr.Contains('not-a-database-url') -or $stderr.Contains('postgres://') -or $stderr.Contains('Listening on')) {
            throw 'Offline command exposed configuration or entered the HTTP path.'
        }
        $checks++
    } finally {
        $process.Dispose()
    }
}
Write-Output "Offline asset/activation command checks passed: $checks. No database, files, HTTP server or worker started."
