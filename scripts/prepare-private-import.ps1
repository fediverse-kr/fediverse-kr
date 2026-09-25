[CmdletBinding()]
param(
    [ValidateSet('New', 'Check', 'Stop')]
    [string] $Mode = 'New',
    [string] $Batch
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$PgBin = 'C:\Program Files\PostgreSQL\17\bin'
$PgPostgres = Join-Path $PgBin 'postgres.exe'
$Role = 'fedkr_import'
$RootNamePrefix = 'cluster-'
$ManifestSchema = 1
$CredentialFilename = 'credential.xml'

. (Join-Path $PSScriptRoot 'private-import-common.ps1')

function Get-Context {
    $context = Get-PrivateImportContext
    foreach ($name in 'initdb.exe', 'pg_ctl.exe', 'psql.exe', 'postgres.exe') {
        if (-not (Test-Path -LiteralPath (Join-Path $PgBin $name) -PathType Leaf)) { Fail 'PostgreSQL 17 tools are unavailable.' }
    }
    $context
}

function Invoke-Native {
    param([string] $Label, [string] $FileName, [string[]] $Arguments, [string] $InputText = '', [string] $PassFile = '', [int] $TimeoutSeconds = 60, [switch] $CaptureOutput, [switch] $KillOwnChildOnTimeout)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $FileName
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { [void]$info.ArgumentList.Add($argument) }
    foreach ($name in @($info.Environment.Keys)) {
        if ($name -like 'PG*' -or $name -like 'FEDKR_*') { [void]$info.Environment.Remove($name) }
    }
    if ($PassFile) { $info.Environment['PGPASSFILE'] = $PassFile }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    try {
        if (-not $process.Start()) { Fail "Failed to start $Label." }
        $outTask = $process.StandardOutput.ReadToEndAsync()
        $errTask = $process.StandardError.ReadToEndAsync()
        if ($InputText) { $process.StandardInput.Write($InputText) }
        $process.StandardInput.Close()
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            # Only initdb/psql opt in to this. pg_ctl may have detached a
            # server, so its timeout must never turn into a generic PG kill.
            if ($KillOwnChildOnTimeout) { try { $process.Kill($true) } catch { } }
            Fail "$Label timed out."
        }
        # pg_ctl may exit after launching postgres while a descendant still
        # owns an inherited pipe. Never wait indefinitely for EOF from that
        # descendant. psql needs its bounded stdout result; start/stop/initdb
        # discard output after this short drain deadline.
        $drained = $false
        try { $drained = [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($outTask, $errTask), 1500) } catch { Fail "$Label output drain failed." }
        if (-not $drained) {
            try { $process.StandardOutput.Close() } catch { }
            try { $process.StandardError.Close() } catch { }
            if ($CaptureOutput) { Fail "$Label output drain timed out." }
        }
        $output = ''
        if ($CaptureOutput) { $output = $outTask.GetAwaiter().GetResult().Trim() }
        [pscustomobject]@{ ExitCode = $process.ExitCode; Output = $output }
    } catch [System.Management.Automation.RuntimeException] { throw }
    catch { Fail "The $Label operation failed." }
    finally { $process.Dispose() }
}

function Write-PrivateFile([string] $Path, [string] $Content, [string] $Sid) {
    $parent = [IO.Path]::GetDirectoryName($Path)
    Assert-NoReparse $parent
    [IO.File]::WriteAllText($Path, $Content, [Text.UTF8Encoding]::new($false))
    Set-PrivateFileAcl $Path $Sid
}

function New-PrivateTempFile([string] $Directory, [string] $Name, [string] $Content, [string] $Sid) {
    Assert-NoReparse $Directory
    Assert-PrivateAcl $Directory $Sid -Shallow
    $path = Join-Path $Directory $Name
    $stream = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes($Content)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    } finally { $stream.Dispose() }
    Set-PrivateFileAcl $path $Sid
    $path
}

function Get-Port {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Parse('127.0.0.1'), 0)
    try { $listener.Start(); return ([Net.IPEndPoint]$listener.LocalEndpoint).Port } finally { $listener.Stop() }
}

function Read-Manifest([pscustomobject] $Context, [string] $RequestedBatch) {
    Assert-Batch $RequestedBatch
    Assert-NoReparse $Context.Base
    Assert-PrivateAcl $Context.Base $Context.Sid -Shallow
    $root = [IO.Path]::GetFullPath((Join-Path $Context.Base ($RootNamePrefix + $RequestedBatch))).TrimEnd('\')
    Assert-NoReparse $root
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { Fail 'Cluster root was not found.' }
    Assert-PrivateAcl $root $Context.Sid
    $manifestPath = Join-Path $root 'manifest.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { Fail 'Manifest was not found.' }
    $manifest = try { Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json } catch { Fail 'Manifest is invalid.' }
    if ($null -eq $manifest -or -not ($manifest.PSObject.Properties.Name -contains 'schema') -or [string]$manifest.schema -ne [string]$ManifestSchema) { Fail 'Manifest schema is invalid.' }
    foreach ($property in 'batch', 'dataDirectory', 'role', 'databases', 'port', 'credentialFile') {
        if (-not ($manifest.PSObject.Properties.Name -contains $property)) { Fail 'Manifest shape is invalid.' }
    }
    if ($manifest.batch -ne $RequestedBatch -or [IO.Path]::GetFullPath([string]$manifest.dataDirectory).TrimEnd('\') -ne $root) { Fail 'Manifest identity does not match the requested batch.' }
    $expectedSnapshot = 'fedkr_snapshot_' + $RequestedBatch
    $expectedRehearsal = 'fedkr_rehearsal_' + $RequestedBatch
    $databases = @($manifest.databases | ForEach-Object { [string]$_ })
    $port = 0
    if ($manifest.role -ne $Role -or $databases.Count -ne 2 -or @($databases | Sort-Object -Unique).Count -ne 2 -or $databases -notcontains $expectedSnapshot -or $databases -notcontains $expectedRehearsal -or $manifest.credentialFile -ne $CredentialFilename -or -not [int]::TryParse([string]$manifest.port, [ref]$port) -or $port -lt 1 -or $port -gt 65535) { Fail 'Manifest contents are invalid.' }
    [pscustomobject]@{ Root = $root; Path = $manifestPath; Data = $manifest; Port = $port; Databases = $databases }
}

function Get-Postmaster([string] $Root, [switch] $AllowStopped) {
    $pidPath = Join-Path $Root 'postmaster.pid'
    if (-not (Test-Path -LiteralPath $pidPath -PathType Leaf)) { if ($AllowStopped) { return $null }; Fail 'PostgreSQL is not running.' }
    $first = (Get-Content -LiteralPath $pidPath -TotalCount 1).Trim()
    if ($first -notmatch '^\d+$') { Fail 'Postmaster handle is invalid.' }
    $processId = [int]$first
    $process = Get-CimInstance -ClassName Win32_Process -Filter "ProcessId = $processId" -ErrorAction SilentlyContinue
    if ($null -eq $process) { if ($AllowStopped) { return $null }; Fail 'Postmaster process is absent.' }
    $expectedExe = [IO.Path]::GetFullPath($PgPostgres)
    $actualExe = if ($process.ExecutablePath) { [IO.Path]::GetFullPath([string]$process.ExecutablePath) } else { '' }
    if ($process.Name -ine 'postgres.exe' -or $actualExe -ne $expectedExe -or [string]::IsNullOrWhiteSpace($process.CommandLine)) { Fail 'Postmaster executable is not PostgreSQL 17 from this private toolset.' }
    # Extract one real -D argument rather than searching for the root as a
    # substring. pg_ctl may normalize Windows separators in that argument.
    $dataArguments = [regex]::Matches([string]$process.CommandLine, '(?i)(?:^|\s)-D(?:\s+|=)(?:"(?<directory>[^"]+)"|(?<directory>\S+))')
    if ($dataArguments.Count -ne 1) { Fail 'Postmaster data directory argument is ambiguous.' }
    try { $actualRoot = [IO.Path]::GetFullPath($dataArguments[0].Groups['directory'].Value).TrimEnd('\') } catch { Fail 'Postmaster data directory argument is invalid.' }
    if ($actualRoot -ne [IO.Path]::GetFullPath($Root).TrimEnd('\')) { Fail 'Postmaster data directory does not exactly match this cluster.' }
    [pscustomobject]@{ Pid = $processId; Process = $process }
}

function Get-ManifestPostmasterId([object] $Manifest) {
    $property = $Manifest.PSObject.Properties['postmasterPid']
    if ($null -eq $property) { return $null }
    $postmasterId = 0
    if (-not [int]::TryParse([string]$property.Value, [ref]$postmasterId) -or $postmasterId -le 0) {
        Fail 'Manifest postmaster handle is invalid.'
    }
    $postmasterId
}

function Invoke-Psql([string] $Exe, [int] $Port, [string] $Database, [string] $PassFile, [string] $Sql, [switch] $CaptureOutput) {
    Invoke-Native -Label 'psql' -FileName $Exe -Arguments @('-X', '--no-password', '-vON_ERROR_STOP=1', '-q', '-A', '-t', '-h', '127.0.0.1', '-p', "$Port", '-U', $Role, '-d', $Database, '-f', '-') -InputText ($Sql + "`n") -PassFile $PassFile -CaptureOutput:$CaptureOutput -KillOwnChildOnTimeout
}

function New-Cluster([pscustomobject] $Context) {
    $batchValue = if ($Batch) { $Batch } else { [Guid]::NewGuid().ToString('N') }
    Assert-Batch $batchValue
    $root = [IO.Path]::GetFullPath((Join-Path $Context.Base ($RootNamePrefix + $batchValue))).TrimEnd('\')
    Assert-NoReparse $Context.Base
    if (Test-Path -LiteralPath $root) { Fail 'The new cluster root already exists.' }
    Set-PrivateDirectoryAcl $Context.Base $Context.Sid
    Assert-NoReparse $Context.Base
    Assert-PrivateAcl $Context.Base $Context.Sid -Shallow
    [IO.Directory]::CreateDirectory($root) | Out-Null
    Set-PrivateDirectoryAcl $root $Context.Sid
    Assert-NoReparse $root
    Assert-PrivateAcl $root $Context.Sid
    $port = Get-Port
    $snapshot = 'fedkr_snapshot_' + $batchValue
    $rehearsal = 'fedkr_rehearsal_' + $batchValue
    $credentialPath = Join-Path $root $CredentialFilename
    $logPath = Join-Path $root 'postgresql-private.log'
    $temporary = @()
    try {
        $password = [Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        # Keep initdb's cleartext password outside its data directory: initdb
        # refuses a non-empty data directory. It is still an exact, private
        # file beneath the already protected local base and is deleted last.
        $initPasswordPath = New-PrivateTempFile $Context.Base ('initdb-' + $batchValue + '-' + [Guid]::NewGuid().ToString('N') + '.pw') ($password + "`n") $Context.Sid
        $temporary += $initPasswordPath
        $init = Invoke-Native -Label 'initdb' -FileName (Join-Path $PgBin 'initdb.exe') -Arguments @('-D', $root, '--username', $Role, '--pwfile', $initPasswordPath, '--auth=scram-sha-256', '--encoding=UTF8', '--no-locale') -KillOwnChildOnTimeout
        if ($init.ExitCode -ne 0) { Fail 'initdb did not complete.' }
        $secure = ConvertTo-SecureString $password -AsPlainText -Force
        [PSCredential]::new($Role, $secure) | Export-Clixml -LiteralPath $credentialPath -Force
        Set-PrivateFileAcl $credentialPath $Context.Sid
        Assert-NoReparse $Context.Base; Assert-NoReparse $root; Assert-PrivateAcl $root $Context.Sid
        $hba = "# Private import cluster: loopback SCRAM only`nhost $snapshot $Role 127.0.0.1/32 scram-sha-256`nhost $rehearsal $Role 127.0.0.1/32 scram-sha-256`nhost postgres $Role 127.0.0.1/32 scram-sha-256`n"
        Write-PrivateFile (Join-Path $root 'pg_hba.conf') $hba $Context.Sid
        $config = @("", '# Private import cluster policy', "listen_addresses = '127.0.0.1'", "port = $port", "password_encryption = 'scram-sha-256'", 'fsync = on', "log_statement = 'none'", 'log_duration = off', 'log_min_duration_statement = -1', 'log_connections = off', 'log_disconnections = off', 'log_parameter_max_length = 0', 'log_parameter_max_length_on_error = 0', "log_min_error_statement = 'panic'", 'log_min_messages = panic', "log_error_verbosity = 'terse'", 'logging_collector = off') -join "`n"
        $confPath = Join-Path $root 'postgresql.conf'
        Write-PrivateFile $confPath (([IO.File]::ReadAllText($confPath) + $config + "`n")) $Context.Sid
        $manifest = [ordered]@{ schema = $ManifestSchema; batch = $batchValue; dataDirectory = $root; role = $Role; databases = @($snapshot, $rehearsal); port = $port; credentialFile = $CredentialFilename; state = 'prepared'; createdUtc = [DateTime]::UtcNow.ToString('O') }
        Write-PrivateFile (Join-Path $root 'manifest.json') (($manifest | ConvertTo-Json -Depth 4) + "`n") $Context.Sid
        Write-PrivateFile $logPath '' $Context.Sid
        Assert-PrivateAcl $root $Context.Sid
        # `-l` prevents the daemon from inheriting the redirected controller
        # pipes. Do not kill pg_ctl on timeout: it might already have detached
        # this new process, and this helper never kills an existing PostgreSQL.
        $start = Invoke-Native -Label 'pg_ctl start' -FileName (Join-Path $PgBin 'pg_ctl.exe') -Arguments @('-D', $root, '-l', $logPath, '-o', "-p $port -h 127.0.0.1", '-w', 'start')
        if ($start.ExitCode -ne 0) { Fail 'PostgreSQL did not start.' }
        Assert-NoReparse $Context.Base; Assert-NoReparse $root; Assert-PrivateAcl $root $Context.Sid
        $postmaster = Get-Postmaster $root
        $passLine = "127.0.0.1:${port}:*:${Role}:$password"
        $passPath = New-PrivateTempFile $Context.Base ('pgpass-' + $batchValue + '-' + [Guid]::NewGuid().ToString('N') + '.conf') ($passLine + "`n") $Context.Sid
        $temporary += $passPath
        foreach ($db in @($snapshot, $rehearsal)) { $create = Invoke-Psql (Join-Path $PgBin 'psql.exe') $port 'postgres' $passPath "CREATE DATABASE $db;"; if ($create.ExitCode -ne 0) { Fail 'Empty rehearsal database creation failed.' } }
        $manifest.state = 'running'; $manifest.postmasterPid = $postmaster.Pid
        Write-PrivateFile (Join-Path $root 'manifest.json') (($manifest | ConvertTo-Json -Depth 4) + "`n") $Context.Sid
        Assert-PrivateAcl $root $Context.Sid
        Write-Output "Status=Created Batch=$batchValue DataDirectory=$root Port=$port Databases=2"
    } finally {
        # Cleartext temp files are the last action in this function and only
        # exact paths created above may be removed.
        for ($index = $temporary.Count - 1; $index -ge 0; $index--) {
            $file = $temporary[$index]
            if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue }
        }
        $password = $null
    }
}

function Check-Cluster([pscustomobject] $Context) {
    if (-not $Batch) { Fail 'Batch is required for Check.' }
    $record = Read-Manifest $Context $Batch
    Assert-NoReparse $Context.Base; Assert-NoReparse $record.Root; Assert-PrivateAcl $record.Root $Context.Sid
    $data = $record.Data; $root = $record.Root; $port = $record.Port; $dbs = $record.Databases
    $pm = Get-Postmaster $root
    $manifestPostmasterId = Get-ManifestPostmasterId $data
    if ($null -ne $manifestPostmasterId -and $manifestPostmasterId -ne $pm.Pid) { Fail 'Manifest postmaster handle does not match the cluster.' }
    Assert-NoReparse $Context.Base; Assert-NoReparse $root; Assert-PrivateAcl $root $Context.Sid
    $hbaActive = @(Get-Content -LiteralPath (Join-Path $root 'pg_hba.conf') | ForEach-Object { $x = $_.Trim(); if ($x -and -not $x.StartsWith('#')) { $x } })
    $expectedHba = @("host $($dbs[0]) $Role 127.0.0.1/32 scram-sha-256", "host $($dbs[1]) $Role 127.0.0.1/32 scram-sha-256", "host postgres $Role 127.0.0.1/32 scram-sha-256")
    $actualHba = @($hbaActive | Sort-Object) -join "`n"
    $policyHba = @($expectedHba | Sort-Object) -join "`n"
    if ($actualHba -ne $policyHba) { Fail 'pg_hba.conf is not the loopback SCRAM policy.' }
    $conf = Get-Content -LiteralPath (Join-Path $root 'postgresql.conf') -Raw
    foreach ($line in "listen_addresses = '127.0.0.1'", "port = $port", "password_encryption = 'scram-sha-256'", 'fsync = on', "log_statement = 'none'", 'log_duration = off', 'log_min_duration_statement = -1', 'log_connections = off', 'log_disconnections = off', 'log_parameter_max_length = 0', 'log_parameter_max_length_on_error = 0', "log_min_error_statement = 'panic'", 'log_min_messages = panic', "log_error_verbosity = 'terse'", 'logging_collector = off') { if ($conf -notmatch [regex]::Escape($line)) { Fail 'postgresql.conf policy is incomplete.' } }
    Assert-NoReparse $Context.Base; Assert-NoReparse $root; Assert-PrivateAcl $root $Context.Sid
    $credentialPath = Join-Path $root $CredentialFilename
    if (-not (Test-Path -LiteralPath $credentialPath -PathType Leaf)) { Fail 'Credential record is missing.' }
    $credential = Import-Clixml -LiteralPath $credentialPath
    $password = $credential.GetNetworkCredential().Password
    $temporary = @()
    try {
        $passPath = New-PrivateTempFile $Context.Base ('pgpass-check-' + $Batch + '-' + [Guid]::NewGuid().ToString('N') + '.conf') ("127.0.0.1:${port}:*:${Role}:$password`n") $Context.Sid
        $temporary += $passPath
        $settings = Invoke-Psql (Join-Path $PgBin 'psql.exe') $port 'postgres' $passPath "SELECT current_setting('data_directory'), current_setting('hba_file'), current_setting('port'), current_setting('listen_addresses'), current_setting('fsync'), current_setting('password_encryption'), current_setting('log_statement'), current_setting('log_duration'), current_setting('log_min_duration_statement'), current_setting('log_connections'), current_setting('log_disconnections'), current_setting('log_parameter_max_length'), current_setting('log_parameter_max_length_on_error'), current_setting('log_min_error_statement'), current_setting('log_min_messages'), current_setting('log_error_verbosity'), current_setting('logging_collector');" -CaptureOutput
        if ($settings.ExitCode -ne 0) { Fail 'Server settings query failed.' }
        $values = $settings.Output.Split('|'); if ($values.Count -lt 17 -or [IO.Path]::GetFullPath($values[0]).TrimEnd('\') -ne $root -or [IO.Path]::GetFullPath($values[1]).TrimEnd('\') -ne (Join-Path $root 'pg_hba.conf') -or $values[2] -ne [string]$port -or $values[3] -ne '127.0.0.1' -or $values[4] -ne 'on' -or $values[5] -ne 'scram-sha-256' -or $values[6] -ne 'none' -or $values[7] -ne 'off' -or $values[8] -ne '-1' -or $values[9] -ne 'off' -or $values[10] -ne 'off' -or $values[11] -ne '0' -or $values[12] -ne '0' -or $values[13] -ne 'panic' -or $values[14] -ne 'panic' -or $values[15] -ne 'terse' -or $values[16] -ne 'off') { Fail 'Effective PostgreSQL settings do not match policy.' }
        $dbResult = Invoke-Psql (Join-Path $PgBin 'psql.exe') $port 'postgres' $passPath "SELECT datname FROM pg_database WHERE datname IN ('$($dbs[0])', '$($dbs[1])') ORDER BY datname;" -CaptureOutput
        if ($dbResult.ExitCode -ne 0 -or (@($dbResult.Output -split "`r?`n" | Where-Object { $_ }) -join ',') -ne (($dbs | Sort-Object) -join ',')) { Fail 'Expected rehearsal databases are missing.' }
        foreach ($db in $dbs) { $empty = Invoke-Psql (Join-Path $PgBin 'psql.exe') $port $db $passPath "SELECT count(*) FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname NOT LIKE 'pg\_%' ESCAPE '\' AND n.nspname <> 'information_schema';" -CaptureOutput; if ($empty.ExitCode -ne 0 -or $empty.Output -ne '0') { Fail 'A rehearsal database is not empty.' } }
        $wrong = [Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        $wrongPath = New-PrivateTempFile $Context.Base ('pgpass-wrong-' + $Batch + '-' + [Guid]::NewGuid().ToString('N') + '.conf') ("127.0.0.1:${port}:*:${Role}:$wrong`n") $Context.Sid
        $temporary += $wrongPath
        $negative = Invoke-Psql (Join-Path $PgBin 'psql.exe') $port 'postgres' $wrongPath 'SELECT 1;'; if ($negative.ExitCode -eq 0) { Fail 'Wrong password was accepted.' }
        Write-Output "Status=OK Batch=$Batch DataDirectory=$root Databases=2 Empty=2 WrongPasswordRejected=True"
    } finally {
        for ($index = $temporary.Count - 1; $index -ge 0; $index--) {
            $file = $temporary[$index]
            if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue }
        }
        $password = $null; $wrong = $null
    }
}

function Stop-Cluster([pscustomobject] $Context) {
    if (-not $Batch) { Fail 'Batch is required for Stop.' }
    $record = Read-Manifest $Context $Batch; Assert-NoReparse $Context.Base; Assert-NoReparse $record.Root; Assert-PrivateAcl $record.Root $Context.Sid
    $pm = Get-Postmaster $record.Root -AllowStopped
    if ($null -eq $pm) { Write-Output "Status=AlreadyStopped Batch=$Batch DataDirectory=$($record.Root)"; return }
    $manifestPostmasterId = Get-ManifestPostmasterId $record.Data
    if ($null -ne $manifestPostmasterId -and $manifestPostmasterId -ne $pm.Pid) { Fail 'Manifest postmaster handle does not match the cluster.' }
    $result = Invoke-Native -Label 'pg_ctl stop' -FileName (Join-Path $PgBin 'pg_ctl.exe') -Arguments @('-D', $record.Root, '-m', 'fast', '-w', 'stop')
    if ($result.ExitCode -ne 0 -or (Get-Postmaster $record.Root -AllowStopped)) { Fail 'PostgreSQL did not stop cleanly.' }
    Write-Output "Status=Stopped Batch=$Batch DataDirectory=$($record.Root)"
}

try {
    $context = Get-Context
    if ($Mode -eq 'New') { New-Cluster $context } elseif ($Mode -eq 'Check') { Check-Cluster $context } else { Stop-Cluster $context }
} catch { Write-Error ('Private import operation failed: ' + $_.Exception.Message); exit 1 }
