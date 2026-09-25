param([ValidateSet('Start','Setup','Test','StopDatabase')][string]$Mode = 'Start')
$ErrorActionPreference = 'Stop'
$projectPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$databasePath = Join-Path $projectPath '.local\postgres'
$databaseLog = Join-Path $projectPath '.local\postgres.log'
$postgresBin = 'C:\Program Files\PostgreSQL\17\bin'
if (!(Test-Path -LiteralPath (Join-Path $postgresBin 'pg_ctl.exe'))) { throw 'PostgreSQL 17 command-line tools are required.' }
if ($Mode -eq 'StopDatabase') {
    if (!(Test-Path -LiteralPath (Join-Path $databasePath 'PG_VERSION'))) { throw 'This workspace has no development cluster.' }
    & "$postgresBin\pg_ctl.exe" -D $databasePath -m fast -w stop
    if ($LASTEXITCODE -ne 0) { throw 'Development cluster stop failed.' }
    exit
}
if (!(Test-Path -LiteralPath (Join-Path $databasePath 'PG_VERSION'))) {
    if ($Mode -ne 'Setup') { throw 'Run ./scripts/dev.ps1 -Mode Setup first.' }
    New-Item -ItemType Directory -Force -Path (Join-Path $projectPath '.local') | Out-Null
    & "$postgresBin\initdb.exe" -D $databasePath -U fedkr_dev --auth=trust --encoding=UTF8 --locale=C
    if ($LASTEXITCODE -ne 0) { throw 'Development cluster initialization failed.' }
}
& "$postgresBin\pg_ctl.exe" -D $databasePath status *> $null
if ($LASTEXITCODE -ne 0) {
    if (Get-NetTCPConnection -State Listen -LocalPort 16439 -ErrorAction SilentlyContinue) { throw 'Port 16439 is occupied; no existing service will be changed.' }
    & "$postgresBin\pg_ctl.exe" -D $databasePath -l $databaseLog -o '-h 127.0.0.1 -p 16439' -w start
    if ($LASTEXITCODE -ne 0) { throw 'Development database start failed.' }
}
$actualPath = (& "$postgresBin\psql.exe" -h 127.0.0.1 -p 16439 -U fedkr_dev -d postgres -Atc 'SHOW data_directory').Trim()
if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($actualPath) -ne $databasePath) { throw 'Development cluster identity mismatch.' }
$env:FEDKR_DATABASE_URL = 'postgresql://fedkr_dev@127.0.0.1:16439/fedkr_dev'
$env:FEDKR_TEST_DATABASE_URL = 'postgresql://fedkr_dev@127.0.0.1:16439/fedkr_test'
$env:FEDKR_PUBLIC_ORIGIN = 'http://127.0.0.1:12239'
Push-Location $projectPath
try {
    if ($Mode -eq 'Setup') {
        foreach ($databaseName in @('fedkr_dev', 'fedkr_test')) {
            $exists = & "$postgresBin\psql.exe" -h 127.0.0.1 -p 16439 -U fedkr_dev -d postgres -Atc "SELECT 1 FROM pg_database WHERE datname='$databaseName'"
            if ($LASTEXITCODE -ne 0) { throw 'Database lookup failed.' }
            if ($exists -ne '1') {
                & "$postgresBin\createdb.exe" -h 127.0.0.1 -p 16439 -U fedkr_dev $databaseName
                if ($LASTEXITCODE -ne 0) { throw 'Development database creation failed.' }
            }
        }
        cargo run --no-default-features --features server --bin fediversekr2 -- --migrate
    } elseif ($Mode -eq 'Test') {
        cargo test --no-default-features --features server --bin fediversekr2 -- --include-ignored --test-threads=1
    } else {
        dx serve --web --fullstack true --bin fediversekr2 --addr 127.0.0.1 --port 12239 --open false --interactive false --watch true
    }
    if ($LASTEXITCODE -ne 0) { throw "Command failed with exit code $LASTEXITCODE" }
} finally { Pop-Location }
