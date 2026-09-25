#requires -Version 7.0
# Only a newly generated empty cluster; no backup paths or credentials accepted.
$ErrorActionPreference = 'Stop'
$setup = Join-Path $PSScriptRoot 'prepare-private-import.ps1'
$probeBatch = [Guid]::NewGuid().ToString('N')
$checks = [Collections.Generic.List[string]]::new()
$attempted = $false
$stopped = $false

function Run-Setup([string]$Mode, [string]$Batch) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $PSHOME 'pwsh.exe'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in @('-NoProfile','-File',$setup,'-Mode',$Mode,'-Batch',$Batch)) {
        $info.ArgumentList.Add($arg)
    }
    $child = [Diagnostics.Process]::Start($info)
    try {
        $stdout = $child.StandardOutput.ReadToEndAsync()
        $stderr = $child.StandardError.ReadToEndAsync()
        if (!$child.WaitForExit(120000)) {
            # Do not kill a PostgreSQL descendant; let the setup's own bounded
            # operation finish, and report an unfinished test instead.
            throw 'Private cluster test exceeded its deadline; inspect the owned batch before retrying.'
        }
        # pg_ctl's detached Windows server may inherit a handle to New's
        # output even after the helper exits. The exit status is authoritative;
        # only Check's result text is needed by this harness.
        $drained = [Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($stdout, $stderr)).Wait(1500)
        if (!$drained -and $Mode -ne 'New') { throw 'Private cluster test output did not close within its deadline.' }
        $out = if ($stdout.IsCompletedSuccessfully) { $stdout.GetAwaiter().GetResult() } else { '' }
        $err = if ($stderr.IsCompletedSuccessfully) { $stderr.GetAwaiter().GetResult() } else { '' }
        if (($out + $err) -match 'postgres(?:ql)?://|<SS>|BEGIN (RSA )?PRIVATE KEY') {
            throw 'Private cluster command output disclosed protected material.'
        }
        return [pscustomobject]@{ Code = $child.ExitCode; Text = $out }
    } finally {
        $child.StandardOutput.Close()
        $child.StandardError.Close()
        $child.Dispose()
    }
}

function Check([string]$Name, [bool]$Condition) {
    if (!$Condition) { throw "Private cluster check failed: $Name. Raw command output withheld." }
    $checks.Add($Name)
}

try {
    Check 'traversal batch rejected' ((Run-Setup 'New' '../not-a-batch').Code -ne 0)
    Check 'unknown batch check rejected' ((Run-Setup 'Check' $probeBatch).Code -ne 0)
    $attempted = $true
    $created = Run-Setup 'New' $probeBatch
    Check 'fresh authenticated cluster created' ($created.Code -eq 0)
    $inspection = Run-Setup 'Check' $probeBatch
    Check 'ACL, effective settings, two empty DBs and bad password inspected' ($inspection.Code -eq 0 -and $inspection.Text.Contains('WrongPasswordRejected=True'))
    Check 'existing cluster cannot be overwritten' ((Run-Setup 'New' $probeBatch).Code -ne 0)
    Check 'existing-root refusal preserves the cluster' ((Run-Setup 'Check' $probeBatch).Code -eq 0)
    $privateBase = Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)) 'fediversekr/private-import'
    $ownedManifest = Join-Path $privateBase "cluster-$probeBatch/manifest.json"
    $original = [IO.File]::ReadAllText($ownedManifest)
    try {
        foreach ($mutation in @('schema', 'credential', 'databases')) {
            $changed = $original | ConvertFrom-Json
            switch ($mutation) {
                'schema' { $changed.schema = 999 }
                'credential' { $changed.credentialFile = '../credential.xml' }
                'databases' { $changed.databases = @('postgres', "fedkr_rehearsal_$probeBatch") }
            }
            [IO.File]::WriteAllText($ownedManifest, ($changed | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
            Check "unexpected manifest $mutation rejected" ((Run-Setup 'Check' $probeBatch).Code -ne 0)
        }
        $changed = $original | ConvertFrom-Json
        $changed.postmasterPid = $PID
        [IO.File]::WriteAllText($ownedManifest, ($changed | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
        Check 'mismatched process identity prevents Stop' ((Run-Setup 'Stop' $probeBatch).Code -ne 0)
    } finally {
        [IO.File]::WriteAllText($ownedManifest, $original, [Text.UTF8Encoding]::new($false))
    }
    Check 'rejected manifest operations preserve the live cluster' ((Run-Setup 'Check' $probeBatch).Code -eq 0)
    Check 'owned cluster stops' ((Run-Setup 'Stop' $probeBatch).Code -eq 0)
    $stopped = $true
    Check 'stopping again is harmless' ((Run-Setup 'Stop' $probeBatch).Code -eq 0)
    Check 'stopped cluster does not claim to pass Check' ((Run-Setup 'Check' $probeBatch).Code -ne 0)
    [pscustomobject]@{ Checks = $checks.Count; Batch = $probeBatch; SyntheticOnly = $true; Stopped = $true; DataRetained = $true } | ConvertTo-Json -Compress
} finally {
    if ($attempted -and !$stopped) {
        $cleanup = Run-Setup 'Stop' $probeBatch
        if ($cleanup.Code -ne 0) { Write-Warning "Owned batch $probeBatch may need inspection; no broad process cleanup was attempted." }
    }
}
