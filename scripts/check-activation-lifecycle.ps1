#requires -Version 7.0
# Real release bundle + synthetic PG/files only. Never accepts a database URL.
# This is not a live AP, browser hydration, TLS or graceful-signal test.
param([string]$Binary = '')
$ErrorActionPreference = 'Stop'
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (!$Binary) { $Binary = Join-Path $workspace 'target/dx/fediversekr2/release/web/server.exe' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$publicDir = Join-Path (Split-Path -Parent $Binary) 'public'
if (!(Test-Path -LiteralPath (Join-Path $publicDir 'index.html') -PathType Leaf)) {
    throw 'A complete dx fullstack release build with adjacent public/index.html is required.'
}
$publicFiles = @(Get-ChildItem -LiteralPath $publicDir -File -Recurse | Where-Object Name -ne 'index.html')
if ($publicFiles.Count -eq 0 -or $publicFiles.Count -gt 1024 -or !($publicFiles | Where-Object Extension -eq '.wasm')) {
    throw 'Expected a bounded Dioxus public bundle including WASM.'
}
$psql = 'C:\Program Files\PostgreSQL\17\bin\psql.exe'
$suffix = [Guid]::NewGuid().ToString('N')
$snapshot = "fedkr_snapshot_$suffix"
$rehearsal = "fedkr_rehearsal_$suffix"
$live = "fedkr_live_$suffix"
$root = Join-Path $workspace ".local/activation-e2e-$suffix"
$sourceDir = Join-Path $root 'source'
$bundleDir = Join-Path $root 'bundle'
$origin = 'https://cutover.example.org'
$databaseOids = [Collections.Generic.HashSet[int]]::new()
$processes = [Collections.Generic.List[object]]::new()
$checks = [Collections.Generic.List[string]]::new()
$httpHandler = [Net.Http.HttpClientHandler]::new()
$httpHandler.UseProxy = $false
$httpHandler.AllowAutoRedirect = $false
$http = [Net.Http.HttpClient]::new($httpHandler)
$http.Timeout = [TimeSpan]::FromSeconds(5)
function Sql([string]$database, [string]$query) {
    if ($database -notin @('fedkr_test',$snapshot,$rehearsal,$live)) { throw 'Unexpected fixture database.' }
    # UTF-8 stdin avoids Windows native argv codepage loss and keeps fixture
    # credentials/private key material out of process command lines.
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName=$psql; $info.UseShellExecute=$false; $info.CreateNoWindow=$true
    $info.RedirectStandardInput=$true; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    $info.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
    $info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    $info.StandardErrorEncoding=[Text.UTF8Encoding]::new($false)
    $info.Environment['PGCLIENTENCODING']='UTF8'
    foreach ($arg in @('-X','-h','127.0.0.1','-p','16439','-U','fedkr_dev','-d',$database,'-v','ON_ERROR_STOP=1','-v','VERBOSITY=sqlstate','-At')) { $info.ArgumentList.Add($arg) }
    $proc=[Diagnostics.Process]::Start($info)
    try {
        $out=$proc.StandardOutput.ReadToEndAsync(); $err=$proc.StandardError.ReadToEndAsync()
        $proc.StandardInput.WriteLine($query); $proc.StandardInput.Close()
        if (!$proc.WaitForExit(30000)) { $proc.Kill($true); throw 'Owned SQL process exceeded its deadline.' }
        $result=$out.GetAwaiter().GetResult(); $errorText=$err.GetAwaiter().GetResult()
        if ($proc.ExitCode -ne 0) {
            $state = [regex]::Match($errorText,'(?:ERROR|FATAL):\s+([0-9A-Z]{5})').Groups[1].Value
            throw "Synthetic lifecycle SQL failed at script line $($MyInvocation.ScriptLineNumber), SQLSTATE=$state; raw output withheld."
        }
        return $result.Trim()
    } finally { if (!$proc.HasExited) { $proc.Kill($true); $proc.WaitForExit() }; $proc.Dispose() }
}
function Check([string]$name, [bool]$value) {
    if (!$value) { throw "Lifecycle check failed: $name" }
    $checks.Add($name)
}
function Assert-Cluster {
    $actual = Sql 'fedkr_test' 'SHOW data_directory'
    if ([IO.Path]::GetFullPath($actual) -ne (Join-Path $workspace '.local/postgres')) {
        throw 'Only the isolated workspace test cluster is allowed.'
    }
}
function Spawn([string[]]$arguments, [hashtable]$environment) {
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Binary
    $info.WorkingDirectory = $root
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($name in @($info.Environment.Keys)) {
        if ($name -like 'FEDKR_*' -or $name -like 'DIOXUS_*') { [void]$info.Environment.Remove($name) }
    }
    foreach ($argument in $arguments) { $info.ArgumentList.Add($argument) }
    foreach ($item in $environment.GetEnumerator()) { $info.Environment[$item.Key] = [string]$item.Value }
    $proc = [Diagnostics.Process]::Start($info)
    $child = [pscustomobject]@{ Process=$proc; Out=$proc.StandardOutput.ReadToEndAsync(); Err=$proc.StandardError.ReadToEndAsync() }
    $processes.Add($child)
    return $child
}
function Finish($child, [int]$expected) {
    if (!$child.Process.WaitForExit(30000)) { throw 'Owned fixture process exceeded its deadline.' }
    $stdout = $child.Out.GetAwaiter().GetResult()
    $stderr = $child.Err.GetAwaiter().GetResult()
    Check 'process exit matches expected outcome' ($child.Process.ExitCode -eq $expected)
    Check 'process output does not disclose keys or database URLs' (-not (($stdout+$stderr) -match 'BEGIN (RSA )?PRIVATE KEY|postgres(ql)?://'))
    return [pscustomobject]@{Out=$stdout;Err=$stderr}
}
function Command([string[]]$arguments, [hashtable]$environment) {
    $result = Finish (Spawn $arguments $environment) 0
    try { return $result.Out | ConvertFrom-Json } catch { throw 'Offline command did not return a JSON report.' }
}
function Stop-Owned($child) {
    # Deliberately a crash/restart test. Never signals a process not spawned here.
    if (!$child.Process.HasExited) { $child.Process.Kill($true); $child.Process.WaitForExit() }
}
function Fetch([string]$path, [string]$token = '', [string]$json = '', [string]$contentType = 'application/json', [switch]$ServerToServer) {
    $method = if ($json) { [Net.Http.HttpMethod]::Post } else { [Net.Http.HttpMethod]::Get }
    # Send the fixture request target verbatim, including deliberately malformed
    # percent escapes. Default .NET Uri would silently turn %ZZ into %25ZZ.
    $uriOptions=[UriCreationOptions]::new()
    $uriOptions.DangerousDisablePathAndQueryCanonicalization=$true
    $uri=[Uri]::new("$base$path",[ref]$uriOptions)
    $message = [Net.Http.HttpRequestMessage]::new($method, $uri)
    if ($token) { [void]$message.Headers.TryAddWithoutValidation('Cookie',"__Host-fedkr_session=$token") }
    if ($json) {
        if (!$ServerToServer) { [void]$message.Headers.TryAddWithoutValidation('Origin',$origin) }
        $message.Content=[Net.Http.StringContent]::new($json,[Text.Encoding]::UTF8,$contentType)
    }
    try {
        $response = $http.SendAsync($message).GetAwaiter().GetResult()
        try {
            return [pscustomobject]@{ Status=[int]$response.StatusCode; Text=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult(); Bytes=$response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult(); Type=[string]$response.Content.Headers.ContentType; Cache=[string]$response.Headers.CacheControl }
        } finally { $response.Dispose() }
    } finally { $message.Dispose() }
}
function Wait-Ready($child) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt 30) {
        if ($child.Process.HasExited) {
            $errors=$child.Err.GetAwaiter().GetResult()
            $category=if ($errors -match 'index.html|ServeConfig|serve config') { 'Dioxus asset configuration' } elseif ($errors -match 'HTTP listener failed') { 'listener bind' } else { 'unclassified startup error' }
            # Only this synthetic process's short panic diagnostic, never full logs.
            $panic=[regex]::Match($errors,"panicked at [^\r\n]+[\r\n]+([^\r\n]+)").Groups[1].Value
            if ($panic -and $panic.Length -lt 300 -and $panic -notmatch 'PRIVATE KEY|postgres(ql)?://|token|password') { $category += ": $panic" }
            throw "Fixture server exited before readiness: $category (exit $($child.Process.ExitCode)); raw logs withheld."
        }
        try { if ((Fetch '/readyz').Status -eq 200) { return } } catch {}
        Start-Sleep -Milliseconds 200
    }
    throw 'Fixture server did not become ready.'
}
function Wait-Sql([string]$query) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt 40) {
        if ((Sql $live $query) -eq 't') { return }
        Start-Sleep -Milliseconds 200
    }
    throw 'In-process worker did not complete the synthetic job.'
}
Assert-Cluster
New-Item -ItemType Directory -Path $root,$sourceDir,$bundleDir | Out-Null
$reserve = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
$reserve.Start(); $port=$reserve.LocalEndpoint.Port; $reserve.Stop()
$base="http://127.0.0.1:$port"
$environment=@{
    FEDKR_IMPORT_SOURCE_URL="postgres://fedkr_dev@127.0.0.1:16439/$snapshot"
    FEDKR_IMPORT_TARGET_URL="postgres://fedkr_dev@127.0.0.1:16439/$rehearsal"
    FEDKR_ACTIVATION_TARGET_URL="postgres://fedkr_dev@127.0.0.1:16439/$live"
    FEDKR_ASSET_SOURCE_DIR=$sourceDir; FEDKR_ASSET_BUNDLE_DIR=$bundleDir
    FEDKR_MEDIA_DIR=$bundleDir; FEDKR_PUBLIC_ORIGIN=$origin
    FEDKR_DATABASE_URL="postgres://fedkr_dev@127.0.0.1:16439/$live"
    IP='127.0.0.1'; PORT=$port
}
try {
    foreach ($name in @($snapshot,$rehearsal)) {
        [void](Sql 'fedkr_test' "CREATE DATABASE $name")
        [void]$databaseOids.Add([int](Sql 'fedkr_test' "SELECT oid FROM pg_database WHERE datname='$name'"))
    }
    [void](Sql $snapshot (Get-Content -Raw -LiteralPath (Join-Path $workspace 'src/backend/db/legacy/fixtures.sql')))
    # Keep the original shortcode and add names whose URL spelling must not be
    # mistaken for a path or for a decoded value. All entries intentionally
    # point at the same legacy bytes so this remains a deduplication fixture.
    [void](Sql $snapshot "UPDATE users SET emojis=jsonb_build_object('smile','emojis/social.example.com/smile.png','blob-cat.@/한국어','emojis/social.example.com/smile.png','.', 'emojis/social.example.com/smile.png','..','emojis/social.example.com/smile.png','a+b','emojis/social.example.com/smile.png','%2F','emojis/social.example.com/smile.png') WHERE id='00000000-0000-0000-0000-000000000001';")
    # No imported site may cause DNS/HTTP requests during runtime startup.
    [void](Sql $snapshot 'UPDATE servers SET is_closed=true;')
    $rsa=[Security.Cryptography.RSA]::Create(2048)
    try {
        $private=$rsa.ExportRSAPrivateKeyPem(); $public=$rsa.ExportSubjectPublicKeyInfoPem()
        [void](Sql $snapshot "INSERT INTO instance_keys VALUES('60000000-0000-0000-0000-000000000001','$public','$private','2025-01-01','2025-01-01')")
    } finally { $rsa.Dispose(); $private=$null }
    $png=[Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=')
    # Fixed 1x1 24-bit BMP (blue pixel, 4-byte padded row), kept inline so the
    # release rehearsal has no generated or user-provided binary dependency.
    $bmp=[byte[]]::new(58)
    $bmp[0]=0x42; $bmp[1]=0x4d; $bmp[2]=0x3a; $bmp[10]=0x36
    $bmp[14]=0x28; $bmp[18]=1; $bmp[22]=1; $bmp[26]=1; $bmp[28]=24
    $bmp[34]=4; $bmp[38]=0x13; $bmp[39]=0x0b; $bmp[42]=0x13; $bmp[43]=0x0b
    $bmp[54]=0xff
    $avatar=[Text.Encoding]::UTF8.GetBytes("<svg xmlns='http://www.w3.org/2000/svg'><circle r='1'/></svg>")
    foreach ($key in @('avatars/fixture.png','favicons/fixture.png','emojis/social.example.com/smile.png','software-logos/fixture.png')) {
        $file=Join-Path $sourceDir $key
        New-Item -ItemType Directory -Path (Split-Path -Parent $file) -Force | Out-Null
        [IO.File]::WriteAllBytes($file,$(if ($key -eq 'avatars/fixture.png') { $avatar } elseif ($key -eq 'software-logos/fixture.png') { $bmp } else { $png }))
    }
    $import=Command @('--legacy-import','--apply') $environment
    Check 'real CLI imports without runtime' ($import.outcome -eq 'applied_quarantined' -and !$import.runtime_enabled -and $import.reserved_members -eq 2)
    $assets=Command @('--legacy-assets','--apply') $environment
    Check 'real CLI preserves four keys and deduplicates shared PNG bytes' ($assets.objects -eq 4 -and $assets.copied_objects -eq 3 -and !$assets.runtime_enabled)
    [void](Sql 'fedkr_test' "ALTER DATABASE $rehearsal RENAME TO $live")
    $blocked=Finish (Spawn @() $environment) 1
    Check 'unapproved renamed copy cannot boot' ($blocked.Err.Contains('cannot be used'))
    Check 'blocked startup created neither jobs nor keys' ((Sql $live "SELECT (SELECT count(*) FROM directory_jobs)=0 AND (SELECT count(*) FROM federation_instance_keys)=1") -eq 't')
    $check=Command @('--legacy-activate') $environment
    Check 'real preflight verifies without approval' ($check.outcome -eq 'verified_not_activated' -and $check.validated_now -and !$check.runtime_enabled)
    $blocked=Finish (Spawn @('--legacy-activate','--apply') $environment) 1
    Check 'apply requires deliberate acknowledgment' ($blocked.Err.Contains('cutover acknowledgment'))
    $environment.FEDKR_ACTIVATION_ACK='source-frozen-and-reviewed'
    $apply=Command @('--legacy-activate','--apply') $environment
    Check 'real CLI commits approval' ($apply.outcome -eq 'activated' -and $apply.runtime_enabled -and $apply.asset_objects -eq 4)
    $wrong=$environment.Clone();$wrong.FEDKR_PUBLIC_ORIGIN='https://wrong.example.org'
    $blocked=Finish (Spawn @() $wrong) 1
    Check 'wrong origin cannot boot' ($blocked.Err.Contains('cannot be used'))
    $missing=$environment.Clone();[void]$missing.Remove('FEDKR_MEDIA_DIR')
    $blocked=Finish (Spawn @() $missing) 1
    Check 'missing store cannot boot' ($blocked.Err.Contains('intact private media store'))
    $server=Spawn @() $environment
    Wait-Ready $server
    Check 'release HTTP and readiness respond' ((Fetch '/healthz').Status -eq 200 -and (Fetch '/readyz').Status -eq 200)
    $landing=Fetch '/'
    Check 'release SSR includes application and hydration entry' ($landing.Status -eq 200 -and $landing.Text.Contains('portal-root') -and $landing.Text.Contains('.js'))
    foreach ($file in $publicFiles) {
        $relative=[IO.Path]::GetRelativePath($publicDir,$file.FullName).Replace('\','/')
        $route='/' + (($relative.Split('/') | ForEach-Object { [Uri]::EscapeDataString($_) }) -join '/')
        $resource=Fetch $route
        $expected=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        Check 'release public file is served byte-exact' ($resource.Status -eq 200 -and [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($resource.Bytes)) -eq $expected)
    }
    $actor=(Fetch '/actor').Text | ConvertFrom-Json
    Check 'imported signing key and configured actor identity are served' ($actor.id -eq "$origin/actor" -and $actor.publicKey.publicKeyPem.Replace("`r`n","`n").Trim() -eq $public.Replace("`r`n","`n").Trim())
    Check 'Phoenix actor host and inbox contract are advertised' ($actor.preferredUsername -eq ([Uri]$origin).Host -and $actor.inbox -eq "$origin/actor/inbox" -and $actor.publicKey.id -eq "$origin/actor#main-key" -and $actor.publicKey.owner -eq "$origin/actor")
    $receipt=Fetch '/actor/inbox' '' '{"@context":"https://www.w3.org/ns/activitystreams","type":"Delete","object":"https://fixture.example/notes/1"}' 'application/activity+json' -ServerToServer
    Check 'server-to-server inbox accepts a no-op without member cookies or Origin' ($receipt.Status -eq 202 -and $receipt.Bytes.Length -eq 0 -and $receipt.Cache.Contains('no-store'))
    Check 'inbox does not turn GET into a successful UI fallback' ((Fetch '/actor/inbox').Status -eq 405)
    Check 'inbox rejects oversized bodies' ((Fetch '/actor/inbox' '' ('x' * 65537) 'application/activity+json' -ServerToServer).Status -eq 413)
    $catalog=(Fetch '/api/public/catalog').Text | ConvertFrom-Json
    Check 'public catalog comes from imported rows' (!$catalog.preview -and $catalog.software.Count -eq 1 -and $catalog.software[0].name -eq 'fixture' -and $catalog.software[0].logo_available)
    $visible=(Fetch '/api/public/server/unknown.example').Text | ConvertFrom-Json
    Check 'visible imported server has a working detail route' (!$visible.preview -and $visible.site.domain -eq 'unknown.example')
    $hidden=(Fetch '/api/public/server/social.example.com').Text | ConvertFrom-Json
    Check 'imported hidden server stays hidden' (!$hidden.preview -and $null -eq $hidden.site)
    $logo=Fetch '/api/public/software-logo/fixture'
    Check 'OpenDAL serves exact preserved BMP bytes' ($logo.Status -eq 200 -and $logo.Type -eq 'image/bmp' -and [Convert]::ToHexString($logo.Bytes) -eq [Convert]::ToHexString($bmp))
    Check 'private media is not anonymously exposed' ((Fetch '/api/member/avatar').Status -eq 401)
    Check 'canonical emoji rejects anonymous requests' ((Fetch '/api/member/emoji?name=smile&v=7').Status -eq 401)
    foreach ($query in @('name=smile%ZZ&v=7','name=&v=7','name=smile&name=smile&v=7')) {
        Check 'invalid canonical emoji query is rejected' ((Fetch "/api/member/emoji?$query").Status -eq 400)
    }
    # Test-only session for an imported fixture UUID, not a public AP proof bypass.
    $tokenBytes=[byte[]]::new(32);[Security.Cryptography.RandomNumberGenerator]::Fill($tokenBytes)
    $token=[Convert]::ToHexString($tokenBytes).ToLowerInvariant()
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($token))).ToLowerInvariant()
    $session=[Guid]::NewGuid().ToString()
    [void](Sql $live "INSERT INTO member_sessions(id,member_id,token_hash,expires_at) VALUES('$session','00000000-0000-0000-0000-000000000001',decode('$hash','hex'),now()+interval '1 hour')")
    $member=(Fetch '/api/member/session' $token).Text | ConvertFrom-Json
    Check 'imported member UUID survives to real HTTP session' ($member.member.id -eq '00000000-0000-0000-0000-000000000001')
    Check 'own imported avatar is served privately' ((Fetch '/api/member/avatar' $token).Status -eq 200)
    $mediaResponse=Fetch '/api/member/profile-media' $token
    $media=$mediaResponse.Text | ConvertFrom-Json
    $expectedEmojiNames=@('smile','blob-cat.@/한국어','.', '..','a+b','%2F')
    $actualEmojiNames=@($media.emojis)
    Check 'profile media preserves every imported emoji shortcode' ($mediaResponse.Status -eq 200 -and (($actualEmojiNames | Sort-Object) -join "`n") -eq (($expectedEmojiNames | Sort-Object) -join "`n"))
    foreach ($name in $expectedEmojiNames) {
        $encoded=[Uri]::EscapeDataString($name)
        $emoji=Fetch "/api/member/emoji?name=$encoded&v=7" $token
        Check 'canonical emoji serves exact PNG bytes and MIME' ($emoji.Status -eq 200 -and $emoji.Type -eq 'image/png' -and [Convert]::ToHexString($emoji.Bytes) -eq [Convert]::ToHexString($png))
    }
    $alias=Fetch '/api/member/emoji/smile' $token
    Check 'legacy emoji path alias remains exact and authorized' ($alias.Status -eq 200 -and $alias.Type -eq 'image/png' -and [Convert]::ToHexString($alias.Bytes) -eq [Convert]::ToHexString($png))
    Check 'legacy emoji path alias rejects anonymous requests' ((Fetch '/api/member/emoji/smile').Status -eq 401)
    $page=Fetch '/account' $token
    Check 'private SSR renders without exposing credential or storage key' ($page.Status -eq 200 -and $page.Cache.Contains('no-store') -and !$page.Text.Contains($token) -and !$page.Text.Contains('avatars/fixture.png'))
    [void](Sql $live "DELETE FROM member_sessions WHERE id='$session'")
    Check 'revoked session immediately loses image access' ((Fetch '/api/member/avatar' $token).Status -eq 401)
    Check 'revoked session immediately loses canonical emoji access' ((Fetch '/api/member/emoji?name=smile&v=7' $token).Status -eq 401)
    [void](Sql $live "INSERT INTO member_sessions(id,member_id,token_hash,expires_at) VALUES('$session','00000000-0000-0000-0000-000000000001',decode('$hash','hex'),now()+interval '1 hour')")
    Check 'real HTTP withdrawal succeeds' ((Fetch '/api/member/withdraw' $token '{"confirmation":"탈퇴"}').Status -eq 200)
    Check 'withdrawn session loses image access' ((Fetch '/api/member/avatar' $token).Status -eq 401)
    Check 'withdrawal atomically retires avatar and emoji mappings' ((Sql $live "SELECT NOT EXISTS(SELECT 1 FROM stored_files WHERE object_key IN ('avatars/fixture.png','emojis/social.example.com/smile.png')) AND NOT EXISTS(SELECT 1 FROM member_users WHERE id='00000000-0000-0000-0000-000000000001')") -eq 't')
    Wait-Sql 'SELECT NOT EXISTS(SELECT 1 FROM media_deletions)'
    $avatarObject=Join-Path $bundleDir ('objects/'+[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($avatar)).ToLowerInvariant())
    Check 'same-process OpenDAL cleanup removes unused avatar bytes' (!(Test-Path -LiteralPath $avatarObject))
    Check 'unrelated software bytes remain readable after withdrawal' ((Fetch '/api/public/software-logo/fixture').Status -eq 200)
    Check 'original export and immutable import inventory remain' ((Test-Path -LiteralPath (Join-Path $sourceDir 'avatars/fixture.png')) -and (Sql $live 'SELECT count(*)=4 FROM legacy_asset_files') -eq 't')
    # This domain is rejected before DNS/HTTP. No remote or loopback health fetch.
    $site=[Guid]::NewGuid().ToString()
    [void](Sql $live "INSERT INTO directory_sites(id,domain,is_hidden) VALUES('$site','127.0.0.1',true)")
    Wait-Sql "SELECT EXISTS(SELECT 1 FROM directory_jobs j JOIN directory_observations o USING(site_id) WHERE j.site_id='$site' AND j.state='complete' AND j.last_error='invalid_domain' AND o.health_error='invalid_domain')"
    Check 'same process schedules and completes a durable job' $true
    Check 'maintenance runs in that process' ((Sql $live 'SELECT count(*)=2 FROM maintenance_schedule') -eq 't')
    [void](Sql $live "UPDATE directory_sites SET is_closed=true WHERE id='$site'")
    Stop-Owned $server
    # Model an interrupted claim, then expire its lease without waiting 120 seconds.
    $job=[Guid]::NewGuid().ToString();$lease=[Guid]::NewGuid().ToString()
    [void](Sql $live "INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,lease_token,lease_until,owner_requested) VALUES('$job','$site',now()-interval '3 minutes','running',1,'$lease',now()-interval '1 second',true)")
    $profileAdmin=[Guid]::NewGuid().ToString();$profileBatch=[Guid]::NewGuid().ToString();$profileAccount=[Guid]::NewGuid().ToString()
    # Only an interrupted synthetic job and a pre-network-rejected identity.
    [void](Sql $live "INSERT INTO member_users(id,display_name) VALUES('$profileAdmin','Profile batch lifecycle fixture'); INSERT INTO member_admin_roles(member_id) VALUES('$profileAdmin'); INSERT INTO member_linked_accounts(id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES('$profileAccount','$profileAdmin','https://127.0.0.1/users/fixture','@fixture@127.0.0.1','Synthetic profile','https://127.0.0.1/users/fixture','activitypub_post','https://127.0.0.1','fixture',now()); INSERT INTO profile_refresh_batches(id,actor_id,state) VALUES('$profileBatch','$profileAdmin','running'); INSERT INTO profile_refresh_jobs(batch_id,member_id,state,attempts,lease_token,lease_until) VALUES('$profileBatch','$profileAdmin','running',1,gen_random_uuid(),now()-interval '1 second');")
    $server=Spawn @() $environment
    Wait-Ready $server
    Check 'restart validates current media instead of requiring retired files' ((Fetch '/api/public/software-logo/fixture').Status -eq 200)
    Wait-Sql "SELECT EXISTS(SELECT 1 FROM directory_jobs WHERE id='$job' AND state='complete' AND attempts=2 AND lease_token IS NULL AND last_error='invalid_domain')"
    Check 'restart recovers an expired synthetic lease without another worker binary' $true
    Wait-Sql "SELECT EXISTS(SELECT 1 FROM profile_refresh_batches b JOIN profile_refresh_jobs j ON j.batch_id=b.id WHERE b.id='$profileBatch' AND b.state='complete' AND j.state='failed' AND j.attempts=2 AND j.lease_token IS NULL)"
    Check 'same process recovers interrupted profile batch after restart' $true
    Check 'profile batch status is private' ((Fetch '/api/member/moderation/profile-batch').Status -eq 401)
    $again=(Fetch '/actor').Text | ConvertFrom-Json
    Check 'restart retains the imported signing key' ($again.publicKey.publicKeyPem -eq $actor.publicKey.publicKeyPem)
    Stop-Owned $server
    $repeat=Command @('--legacy-activate','--apply') $environment
    Check 'retry after runtime writes reports prior approval without replaying snapshot' ($repeat.outcome -eq 'already_activated' -and !$repeat.validated_now)
    Check 'live worker history survives activation retry' ((Sql $live "SELECT EXISTS(SELECT 1 FROM directory_jobs WHERE id='$job' AND attempts=2)") -eq 't')
    # Startup must catch changed bytes, not merely accept the same directory name.
    $object=Join-Path $bundleDir ('objects/'+[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($png)).ToLowerInvariant())
    [IO.File]::WriteAllBytes($object,[Text.Encoding]::UTF8.GetBytes('synthetic changed bytes'))
    $blocked=Finish (Spawn @() $environment) 1
    Check 'tampered store cannot boot after approval' ($blocked.Err.Contains('intact private media store'))
    Write-Output "Release activation lifecycle checks passed: $($checks.Count), including $($publicFiles.Count) public files. Synthetic PG/files only; worker exercised through pre-network rejection."
} finally {
    foreach ($child in $processes) { Stop-Owned $child; $child.Process.Dispose() }
    $http.Dispose()
    Assert-Cluster
    foreach ($name in @($live,$rehearsal,$snapshot)) {
        $oid=Sql 'fedkr_test' "SELECT oid FROM pg_database WHERE datname='$name'"
        if ($oid) {
            if (!$databaseOids.Contains([int]$oid)) { throw 'Refusing to remove a database not created by this run.' }
            [void](Sql 'fedkr_test' "DROP DATABASE $name")
        }
    }
    $item=Get-Item -LiteralPath $root
    if ($item.FullName -ne [IO.Path]::GetFullPath($root) -or $item.Name -ne "activation-e2e-$suffix" -or $item.Parent.FullName -ne (Join-Path $workspace '.local') -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing unsafe temporary directory cleanup.' }
    Remove-Item -LiteralPath $root -Recurse -Force
    Write-Output 'Removed only the generated lifecycle databases/files and stopped owned test processes; fixtures are reproducible.'
}
