#requires -Version 7.4
# Generates public synthetic data only. Does not accept any real archive or password.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$helper = Join-Path $PSScriptRoot 'prepare-private-archive.ps1'
$testRoot = Join-Path $repo ('.local/archive-check-' + [Guid]::NewGuid().ToString('N'))
$checks = [Collections.Generic.List[string]]::new()
$ownedBatches = [Collections.Generic.HashSet[string]]::new()
. (Join-Path $PSScriptRoot 'private-import-common.ps1')
$context = Get-PrivateImportContext

function Check([string] $Name, [bool] $Condition) {
    if (-not $Condition) { throw "Synthetic archive check failed: $Name. Child output withheld." }
    $checks.Add($Name)
}

function Quote-Ps([string] $Value) { "'" + $Value.Replace("'", "''") + "'" }

function Run-Archive([string] $Fixture = '', [string] $Batch = '', [switch] $WrongPassword) {
    if ($Fixture) {
        $fixturePath = Join-Path $testRoot $Fixture
        # A fixed public fixture password is not a real credential. The helper
        # receives a SecureString; no password parameter is added to its CLI.
        $publicPassword = if ($WrongPassword) { 'WRONG_PUBLIC_TEST_PASSWORD' } else { 'PUBLIC_TEST_PASSWORD' }
        $command = '$p = ConvertTo-SecureString ' + (Quote-Ps $publicPassword) + ' -AsPlainText -Force; & ' + (Quote-Ps $helper) + ' -Mode Extract -ArchivePath ' + (Quote-Ps $fixturePath) + ' -Password $p'
    } else {
        $command = '& ' + (Quote-Ps $helper) + ' -Mode Check -Batch ' + (Quote-Ps $Batch)
    }
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = Join-Path $PSHOME 'pwsh.exe'
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in @('-NoProfile', '-NonInteractive', '-EncodedCommand', [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command)))) {
        $info.ArgumentList.Add($argument)
    }
    $child = [Diagnostics.Process]::Start($info)
    try {
        $stdout = $child.StandardOutput.ReadToEndAsync()
        $stderr = $child.StandardError.ReadToEndAsync()
        if (-not $child.WaitForExit(120000)) {
            # This specific child runs only the archive reader, never a PG
            # server. Stop it on timeout; retain any private partial output.
            $child.Kill()
            throw 'Synthetic archive helper timed out; partial output retained.'
        }
        if (-not [Threading.Tasks.Task]::WhenAll([Threading.Tasks.Task[]]@($stdout, $stderr)).Wait(2000)) {
            throw 'Synthetic archive output did not close.'
        }
        $out = $stdout.GetAwaiter().GetResult()
        $err = $stderr.GetAwaiter().GetResult()
        $combined = $out + $err
        if ($combined -match 'PUBLIC_TEST_PASSWORD|SYNTHETIC|database\.dump|restore\.sql|tag-check\.txt|postgres(?:ql)?://|BEGIN (RSA )?PRIVATE KEY' -or $combined.Contains($testRoot)) {
            throw 'Archive helper exposed fixture material or a source path. Output withheld.'
        }
        $resultBatch = ''
        if ($out -match 'Batch=([0-9a-f]{32})') {
            $resultBatch = $Matches[1]
            if ($Fixture) { [void]$ownedBatches.Add($resultBatch) }
        }
        [pscustomobject]@{ Code = $child.ExitCode; Text = $out; Batch = $resultBatch }
    } finally {
        $child.Dispose()
    }
}

function Owned-Root([string] $Batch) {
    Assert-Batch $Batch
    if (-not $ownedBatches.Contains($Batch)) { throw 'Not a synthetic batch owned by this run.' }
    $root = [IO.Path]::GetFullPath((Join-Path $context.Base ('archive-' + $Batch)))
    if (-not $root.StartsWith($context.Base + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid owned path.' }
    Assert-NoReparse $root
    $root
}

function Successful-Extract([string] $Fixture) {
    $result = Run-Archive -Fixture $Fixture
    Check "extract $Fixture" ($result.Code -eq 0 -and $result.Text.Contains('Status=Complete') -and $result.Batch.Length -eq 32)
    $result
}

Add-Type -Path (Join-Path $PSScriptRoot 'private-archive-stream.cs')
$timer = [Diagnostics.Stopwatch]::StartNew()
$destination = [IO.MemoryStream]::new()
$bounded = [Fedkr.PrivateBackup.BoundedWriteStream]::new($destination, 3, $timer, [TimeSpan]::FromMinutes(1))
try {
    $bounded.Write([byte[]]@(1, 2, 3), 0, 3)
    Check 'bounded stream hashes all written bytes' ($bounded.BytesWritten -eq 3 -and $bounded.HexSha256() -eq '039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81')
    $rejected = $false
    try { $bounded.WriteByte(4) } catch { $rejected = $true }
    Check 'bounded stream refuses excess before writing' ($rejected -and $destination.Length -eq 3)
} finally { $bounded.Dispose() }
Check 'bounded stream leaves caller destination open' $destination.CanWrite
$destination.Dispose()
$expiredDestination = [IO.MemoryStream]::new()
$expired = [Fedkr.PrivateBackup.BoundedWriteStream]::new($expiredDestination, 1, $timer, [TimeSpan]::FromTicks(1))
try {
    $rejected = $false
    try { $expired.WriteByte(1) } catch { $rejected = $true }
    Check 'bounded stream refuses expired budget' ($rejected -and $expired.BytesWritten -eq 0)
} finally { $expired.Dispose(); $expiredDestination.Dispose() }

& python -B (Join-Path $PSScriptRoot 'make-archive-fixtures.py') --output $testRoot | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Synthetic fixture generation failed.' }
$sourceHashes = @{}
foreach ($file in Get-ChildItem -LiteralPath $testRoot -Filter '*.zip') {
    $sourceHashes[$file.Name] = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
}

Check 'invalid batch refused' ((Run-Archive -Batch '../not-a-batch').Code -ne 0)
Check 'unknown batch refused' ((Run-Archive -Batch ([Guid]::NewGuid().ToString('N'))).Code -ne 0)
$normal = Successful-Extract 'normal-aes256.zip'
Check 'complete extraction passes independent Check' ((Run-Archive -Batch $normal.Batch).Code -eq 0)
$normalRoot = Owned-Root $normal.Batch
Assert-PrivateAcl $normalRoot $context.Sid
Check 'private ACL verified for all outputs' $true
$manifestPath = Join-Path $normalRoot 'manifest.json'
$originalManifest = [IO.File]::ReadAllText($manifestPath)
$manifest = $originalManifest | ConvertFrom-Json
Check 'private inventory has six file hashes' (@($manifest.entries).Count -eq 6)
Check 'private inventory records source digest' ($manifest.sourceArchiveSha256 -eq $sourceHashes['normal-aes256.zip'].ToLowerInvariant())
Check 'private inventory excludes source path and password' (-not $originalManifest.Contains($testRoot) -and -not $originalManifest.Contains('PUBLIC_TEST_PASSWORD'))
$dumpPath = Join-Path $normalRoot 'files/database.dump'
Check 'synthetic dump extracted exactly' ([Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($dumpPath)) -eq "PGDMP$([char]1)$([char]0)SYNTHETIC-DATABASE-DUMP`n")
foreach ($mutation in @('schema', 'obsoleteSchema', 'state', 'batch', 'sourceHash', 'entryPath', 'entrySize', 'entryHash', 'counts')) {
    try {
        $changed = $originalManifest | ConvertFrom-Json
        switch ($mutation) {
            'schema' { $changed.schema = 999 }
            'obsoleteSchema' { $changed.schema = 1 }
            'state' { $changed.state = 'failed' }
            'batch' { $changed.batch = [Guid]::NewGuid().ToString('N') }
            'sourceHash' { $changed.sourceArchiveSha256 = 'not-a-hash' }
            'entryPath' { $changed.entries[0].path = '../escape.txt' }
            'entrySize' { $changed.entries[0].size += 1 }
            'entryHash' { $changed.entries[0].sha256 = '0' * 64 }
            'counts' { $changed.counts.files += 1 }
        }
        [IO.File]::WriteAllText($manifestPath, ($changed | ConvertTo-Json -Depth 8), [Text.UTF8Encoding]::new($false))
        Check "manifest $mutation refused" ((Run-Archive -Batch $normal.Batch).Code -ne 0)
    } finally { [IO.File]::WriteAllText($manifestPath, $originalManifest, [Text.UTF8Encoding]::new($false)) }
}
Check 'Check does not mutate restored manifest' ((Run-Archive -Batch $normal.Batch).Code -eq 0 -and [IO.File]::ReadAllText($manifestPath) -ceq $originalManifest)

$empty = Successful-Extract 'empty-entry.zip'
Check 'empty encrypted entry passes Check' ((Run-Archive -Batch $empty.Batch).Code -eq 0)
$nested = Successful-Extract 'nested-and-directories.zip'
Check 'nested Unicode files and explicit directories pass Check' ((Run-Archive -Batch $nested.Batch).Code -eq 0)
$nestedRoot = Owned-Root $nested.Batch
Check 'explicit empty directory preserved' (Test-Path -LiteralPath (Join-Path $nestedRoot 'files/empty') -PathType Container)

$badPassword = Run-Archive -Fixture 'normal-aes256.zip' -WrongPassword
Check 'incorrect password rejected' ($badPassword.Code -ne 0 -and -not $badPassword.Text.Contains('Status=Complete'))
foreach ($fixture in @('tampered-aes-tag.zip', 'traversal-name.zip', 'absolute-name.zip', 'backslash-name.zip', 'case-collision.zip', 'symlink-entry.zip', 'ratio-over-100.zip', 'unsupported-bzip2.zip', 'file-directory-conflict.zip', 'unencrypted.zip', 'reserved-name.zip', 'too-many-entries.zip')) {
    if (-not (Test-Path -LiteralPath (Join-Path $testRoot $fixture))) { throw 'A required hostile fixture is missing.' }
    $refused = Run-Archive -Fixture $fixture
    Check "refuse $fixture" ($refused.Code -ne 0 -and -not $refused.Text.Contains('Status=Complete'))
    if ($refused.Batch) {
        $failedRoot = Owned-Root $refused.Batch
        $failedManifest = [IO.File]::ReadAllText((Join-Path $failedRoot 'manifest.json')) | ConvertFrom-Json
        Check "failure recorded for $fixture" ($failedManifest.state -eq 'failed')
        Check "failed output never passes Check for $fixture" ((Run-Archive -Batch $refused.Batch).Code -ne 0)
    }
}

$changedOutput = Successful-Extract 'normal-aes256.zip'
$changedRoot = Owned-Root $changedOutput.Batch
$changedFile = Join-Path $changedRoot 'files/notes.txt'
[IO.File]::AppendAllText($changedFile, 'SYNTHETIC MUTATION', [Text.UTF8Encoding]::new($false))
Check 'altered extracted bytes refused' ((Run-Archive -Batch $changedOutput.Batch).Code -ne 0)

$addedOutput = Successful-Extract 'normal-aes256.zip'
$addedRoot = Owned-Root $addedOutput.Batch
$extraFile = [IO.File]::Open((Join-Path $addedRoot 'files/extra.txt'), [IO.FileMode]::CreateNew)
$extraFile.Dispose()
Check 'unlisted extracted file refused' ((Run-Archive -Batch $addedOutput.Batch).Code -ne 0)

$missingOutput = Successful-Extract 'normal-aes256.zip'
$missingRoot = Owned-Root $missingOutput.Batch
$missingSource = [IO.Path]::GetFullPath((Join-Path $missingRoot 'files/notes.txt'))
$missingDestination = [IO.Path]::GetFullPath((Join-Path $testRoot 'held-synthetic-notes.txt'))
if (-not $missingSource.StartsWith($missingRoot + '\') -or -not $missingDestination.StartsWith($testRoot + '\')) { throw 'Invalid synthetic move boundaries.' }
Move-Item -LiteralPath $missingSource -Destination $missingDestination -ErrorAction Stop
Check 'missing extracted file refused' ((Run-Archive -Batch $missingOutput.Batch).Code -ne 0)

foreach ($name in $sourceHashes.Keys) {
    Check "source untouched: $name" ((Get-FileHash -LiteralPath (Join-Path $testRoot $name) -Algorithm SHA256).Hash -eq $sourceHashes[$name])
}
[pscustomobject]@{
    Checks = $checks.Count
    SyntheticOnly = $true
    SourceArchivesUnchanged = $true
    OutputsRetained = $true
    FixtureDirectory = $testRoot
    Batches = @($ownedBatches)
} | ConvertTo-Json -Compress
