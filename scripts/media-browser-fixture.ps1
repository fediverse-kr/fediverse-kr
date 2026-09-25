# Synthetic image fixture only. Never restores or reads the production archive.
param([ValidateSet('Seed','Clean')][string]$Mode = 'Seed', [switch]$IncludeProfileBatch, [switch]$LegacyShortcodes)
$ErrorActionPreference = 'Stop'
$projectPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$fixturePath = Join-Path $projectPath '.local\media-browser-fixture.json'
$baseFixturePath = Join-Path $projectPath '.local\browser-fixture.json'
$mediaRoot = Join-Path $projectPath '.local\media'
$objectsRoot = Join-Path $mediaRoot 'objects'
$psqlPath = 'C:\Program Files\PostgreSQL\17\bin\psql.exe'
function Invoke-FixtureSql([string]$query) {
    $result = & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -Atc $query
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic media fixture database operation failed.' }
    return $result
}
$actualCluster = (Invoke-FixtureSql 'SHOW data_directory').Trim()
if ([IO.Path]::GetFullPath($actualCluster) -ne (Join-Path $projectPath '.local\postgres')) { throw 'Workspace fixture cluster required.' }
function Assert-LocalDirectory([string]$path) {
    $absolute = [IO.Path]::GetFullPath($path)
    if ($absolute -ne $mediaRoot -and $absolute -ne $objectsRoot) { throw 'Unexpected media fixture directory.' }
    if (!(Test-Path -LiteralPath $absolute)) { New-Item -ItemType Directory -Path $absolute | Out-Null }
    $item = Get-Item -LiteralPath $absolute
    if (!$item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing a linked media fixture directory.' }
}
Assert-LocalDirectory $mediaRoot
Assert-LocalDirectory $objectsRoot
if ($Mode -eq 'Clean') {
    if (!(Test-Path -LiteralPath $fixturePath)) { throw 'No media fixture marker.' }
    $fixture = Get-Content -Raw -LiteralPath $fixturePath | ConvertFrom-Json
    $memberId = [Guid]::Parse($fixture.member_id).ToString()
    $siteId = [Guid]::Parse($fixture.site_id).ToString()
    $baseFixture = Get-Content -Raw -LiteralPath $baseFixturePath | ConvertFrom-Json
    if ($baseFixture.member_id -ne $memberId -or $baseFixture.site_id -ne $siteId) { throw 'Base fixture changed; manual inspection required.' }
    Invoke-FixtureSql "BEGIN; DELETE FROM legacy_members WHERE id='$memberId' AND fediverse_domain='media-fixture.example.org'; DELETE FROM legacy_sites WHERE id='$siteId' AND domain='browser-$memberId.example.org'; DELETE FROM catalog_software WHERE name='browser-media-$memberId' AND display_name='Media browser fixture'; DELETE FROM stored_files WHERE object_key IN ('avatars/media-$memberId.svg','emojis/media-$memberId/wave.png','software-logos/media-$memberId.svg','favicons/media-$memberId.png'); UPDATE member_users SET display_name='Browser test fixture' WHERE id='$memberId' AND (display_name='Browser test fixture :wave:' OR display_name='Browser test fixture :wave: :blob-cat.@/'||chr(54620)||chr(44397)||chr(50612)||': :..:'); COMMIT;" | Out-Null
    foreach ($file in $fixture.files) {
        if (!$file.created) { continue }
        $hash = [string]$file.hash
        if ($hash -cnotmatch '^[a-f0-9]{64}$') { throw 'Invalid recorded object hash.' }
        $path = [IO.Path]::GetFullPath((Join-Path $objectsRoot $hash))
        if ([IO.Path]::GetDirectoryName($path) -ne $objectsRoot) { throw 'Object outside fixture directory.' }
        if (!(Test-Path -LiteralPath $path)) { continue }
        if ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing linked fixture object.' }
        if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hash) { throw 'Fixture object changed; retaining it for inspection.' }
        if ([int](Invoke-FixtureSql "SELECT count(*) FROM stored_files WHERE sha256='$hash'") -ne 0) { continue }
        Remove-Item -LiteralPath $path
    }
    Remove-Item -LiteralPath $fixturePath
    & (Join-Path $PSScriptRoot 'member-browser-fixture.ps1') -Mode Clean
    Write-Output 'Removed only synthetic media rows and unshared fixture files; reproducible with Seed.'
    exit
}
if ((Test-Path -LiteralPath $fixturePath) -or (Test-Path -LiteralPath $baseFixturePath)) { throw 'Clean the existing fixture before seeding media.' }
if ((Invoke-FixtureSql "SELECT to_regclass('public.stored_files') IS NOT NULL") -ne 't') { throw 'Start the current server to apply embedded migrations first.' }
& (Join-Path $PSScriptRoot 'member-browser-fixture.ps1') -Mode Seed -IncludeSite -IncludeCatalog -IncludeProfileBatch:$IncludeProfileBatch
$baseFixture = Get-Content -Raw -LiteralPath $baseFixturePath | ConvertFrom-Json
$memberId = [Guid]::Parse($baseFixture.member_id).ToString()
$siteId = [Guid]::Parse($baseFixture.site_id).ToString()
$kindId = [long]$baseFixture.catalog_kind_id
if ($kindId -ge 0) { throw 'Negative synthetic category ID required.' }
$png = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=')
$avatar = [Text.Encoding]::UTF8.GetBytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64' width='64' height='64'><!-- $memberId --><rect width='64' height='64' fill='#e2e8ff'/><circle cx='32' cy='26' r='12' fill='#5965bb'/><path d='M10 64Q10 40 32 40Q54 40 54 64' fill='#5965bb'/></svg>")
# Deliberately active SVG sentinel: the browser must block both script and the external image.
$logo = [Text.Encoding]::UTF8.GetBytes("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64' width='64' height='64'><!-- $memberId --><style>rect{fill:#deeee4}circle{fill:#247a51}</style><rect width='64' height='64' rx='12'/><circle cx='32' cy='32' r='17'/><script>window.__media_executed=true;fetch('/__media_probe')</script><image href='https://media-fixture.invalid/blocked.png' width='1' height='1'/></svg>")
$objects = @(
    @{key="avatars/media-$memberId.svg"; bytes=$avatar},
    @{key="emojis/media-$memberId/wave.png"; bytes=$png},
    @{key="software-logos/media-$memberId.svg"; bytes=$logo},
    @{key="favicons/media-$memberId.png"; bytes=$png}
)
if ($LegacyShortcodes) {
    # A hand-built, uncompressed 1x1 24-bit BMP. The original .png logical key
    # intentionally does not determine the response's MIME type.
    $bmp = [byte[]]::new(58)
    $bmp[0]=0x42; $bmp[1]=0x4d; $bmp[2]=58; $bmp[10]=54
    $bmp[14]=40; $bmp[18]=1; $bmp[22]=1; $bmp[26]=1; $bmp[28]=24; $bmp[34]=4
    $bmp[54]=0xbb; $bmp[55]=0x65; $bmp[56]=0x59
    $objects[1].bytes = $bmp
}
$files = @()
foreach ($object in $objects) {
    $object.hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($object.bytes)).ToLowerInvariant()
    $path = Join-Path $objectsRoot $object.hash
    if ($files.hash -contains $object.hash) { continue }
    $files += @{hash=$object.hash; created=!(Test-Path -LiteralPath $path)}
}
@{member_id=$memberId;site_id=$siteId;files=$files} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $fixturePath -Encoding utf8
foreach ($object in $objects) {
    $path = Join-Path $objectsRoot $object.hash
    if (Test-Path -LiteralPath $path) {
        if (((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint) -or (Get-FileHash -LiteralPath $path).Hash.ToLowerInvariant() -ne $object.hash) { throw 'Existing fixture object is not the expected bytes.' }
    } else {
        $stream = [IO.File]::Open($path,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        try { $stream.Write($object.bytes) } finally { $stream.Dispose() }
    }
}
$sql = 'BEGIN;'
foreach ($object in $objects) { $sql += "INSERT INTO stored_files(object_key,sha256,byte_count) VALUES('$($object.key)','$($object.hash)',$($object.bytes.Length));" }
$sql += "INSERT INTO legacy_members(id,fediverse_handle,fediverse_domain,display_name,avatar_key,emojis,inserted_at,updated_at) VALUES('$memberId','@fixture@media-fixture.example.org','media-fixture.example.org','Browser test fixture :wave:','avatars/media-$memberId.svg',jsonb_build_object('wave','emojis/media-$memberId/wave.png'),now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC'); INSERT INTO legacy_sites(id,domain,favicon_key,is_force_hidden,inserted_at,updated_at) VALUES('$siteId','browser-$memberId.example.org','favicons/media-$memberId.png',false,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC'); INSERT INTO catalog_software(id,name,display_name,description,categories,logo_key,is_featured,display_order,inserted_at,updated_at) VALUES($kindId,'browser-media-$memberId','Media browser fixture','Image rendering fixture only.',ARRAY['browser-kind-$memberId'],'software-logos/media-$memberId.svg',false,0,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC'); UPDATE member_users SET display_name='Browser test fixture :wave:' WHERE id='$memberId' AND display_name='Browser test fixture'; COMMIT;"
if ($LegacyShortcodes) {
    # chr keeps this fixture independent of the Windows native argv code page.
    $logicalName="'blob-cat.@/'||chr(54620)||chr(44397)||chr(50612)"
    $sql=$sql.Replace('COMMIT;', "UPDATE legacy_members SET emojis=emojis||jsonb_build_object($logicalName,'emojis/media-$memberId/wave.png','.','emojis/media-$memberId/wave.png','..','emojis/media-$memberId/wave.png','a+b','emojis/media-$memberId/wave.png','%2F','emojis/media-$memberId/wave.png') WHERE id='$memberId'; UPDATE member_users SET display_name='Browser test fixture :wave: :'||($logicalName)||': :..:' WHERE id='$memberId'; COMMIT;")
}
Invoke-FixtureSql $sql | Out-Null
Write-Output 'Synthetic image fixture created. No production images, import activation or remote reads.'
