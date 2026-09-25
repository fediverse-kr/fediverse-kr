# Isolated browser test setup only, NOT an application signup/auth endpoint.
# AP ownership itself is exercised by backend::flow tests using the real verifier.
param([ValidateSet('Seed','Clean')][string]$Mode = 'Seed', [switch]$IncludeSite, [switch]$IncludeIcon, [switch]$IncludeSvgIcon, [switch]$IncludeCatalog, [switch]$IncludeCommunity, [switch]$IncludeRegistration, [switch]$IncludeModeration, [switch]$IncludeCatalogAdmin, [switch]$IncludeHealth, [switch]$IncludeOwnComments, [switch]$IncludeProfileBatch, [switch]$IncludeWorkerMonitor)
$ErrorActionPreference = 'Stop'
$projectPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$fixturePath = Join-Path $projectPath '.local\browser-fixture.json'
$browserStatePath = Join-Path $projectPath '.local\browser-state.json'
$expectedCluster = Join-Path $projectPath '.local\postgres'
$psqlPath = 'C:\Program Files\PostgreSQL\17\bin\psql.exe'
$actualCluster = (& $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -Atc 'SHOW data_directory').Trim()
if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($actualCluster) -ne $expectedCluster) { throw 'Refusing to change a database outside the workspace fixture cluster.' }
if ($Mode -eq 'Clean') {
    if (!(Test-Path -LiteralPath $fixturePath)) { throw 'No browser fixture to clean.' }
    $fixture = Get-Content -Raw -LiteralPath $fixturePath | ConvertFrom-Json
    $memberId = [Guid]::Parse($fixture.member_id).ToString()
    if ($fixture.worker_site_id) {
        $workerSiteId = [Guid]::Parse($fixture.worker_site_id).ToString()
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM directory_sites WHERE id='$workerSiteId' AND domain='worker-$memberId.example.org' AND name='Browser worker fixture';" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Worker monitor fixture cleanup failed.' }
    }
    # Batch jobs have no target FK so progress survives a real withdrawal. Only
    # remove batches explicitly started by this recorded synthetic member.
    & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM profile_refresh_batches WHERE actor_id='$memberId' AND EXISTS(SELECT 1 FROM member_users WHERE id='$memberId' AND display_name='Browser test fixture');"
    if ($LASTEXITCODE -ne 0) { throw 'Profile batch fixture cleanup failed.' }
    foreach ($extra in $fixture.profile_member_ids) {
        $extraId = [Guid]::Parse($extra).ToString()
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM member_users WHERE id='$extraId' AND display_name='Browser profile batch fixture';" | Out-Null
        if ($LASTEXITCODE -ne 0) { throw 'Profile target fixture cleanup failed.' }
    }
    if ($fixture.catalog_admin) {
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "BEGIN; DELETE FROM catalog_categories c WHERE name='browser-extra-kind-$memberId' AND EXISTS(SELECT 1 FROM catalog_admin_events e WHERE e.category_id=c.id AND e.action='category_create' AND e.actor_id='$memberId'); DELETE FROM moderation_events WHERE target_id='$memberId' OR actor_id='$memberId'; COMMIT;"
        if ($LASTEXITCODE -ne 0) { throw 'Catalog administrator fixture cleanup failed.' }
    }
    if ($fixture.moderation) {
        $otherId=[Guid]::Parse($fixture.community_member_id).ToString()
        $siteId=[Guid]::Parse($fixture.site_id).ToString()
        $reportId=[Guid]::Parse($fixture.report_id).ToString()
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM moderation_events WHERE target_id IN ('$memberId','$otherId','$siteId') OR report_id='$reportId';"
        if ($LASTEXITCODE -ne 0) { throw 'Moderation fixture cleanup failed.' }
    }
    if ($fixture.catalog_kind_id) {
        $kindId = [long]$fixture.catalog_kind_id
        if ($kindId -ge 0) { throw 'Refusing non-fixture category ID.' }
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "BEGIN; DELETE FROM catalog_software s WHERE name='browser-$memberId' AND EXISTS(SELECT 1 FROM catalog_software_edits e WHERE e.software_id=s.id AND e.revision=1 AND e.action='create' AND e.actor_id='$memberId'); DELETE FROM catalog_categories WHERE id=$kindId AND name='browser-kind-$memberId'; COMMIT;"
        if ($LASTEXITCODE -ne 0) { throw 'Catalog fixture cleanup failed.' }
    }
    if ($fixture.site_id) {
        $siteId = [Guid]::Parse($fixture.site_id).ToString()
        if ($fixture.community_member_id) {
            $reportClause = if ($fixture.report_id) { "DELETE FROM community_reports WHERE id='$( [Guid]::Parse($fixture.report_id).ToString() )' OR comment_id IN (SELECT c.id FROM community_comments c JOIN directory_sites s ON s.id=c.server_id WHERE s.id='$siteId' AND s.domain='browser-$memberId.example.org');" } else { "DELETE FROM community_reports WHERE comment_id IN (SELECT c.id FROM community_comments c JOIN directory_sites s ON s.id=c.server_id WHERE s.id='$siteId' AND s.domain='browser-$memberId.example.org');" }
            & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "BEGIN; $reportClause DELETE FROM community_comments WHERE server_id IN (SELECT id FROM directory_sites WHERE id='$siteId' AND domain='browser-$memberId.example.org'); COMMIT;"
            if ($LASTEXITCODE -ne 0) { throw 'Community fixture cleanup failed.' }
        }
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM directory_sites WHERE id='$siteId' AND domain IN ('browser-$memberId.example','browser-$memberId.example.org');"
        if ($LASTEXITCODE -ne 0) { throw 'Site fixture cleanup failed.' }
    }
    if ($fixture.community_member_id) {
        $otherId=[Guid]::Parse($fixture.community_member_id).ToString()
        & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM member_users WHERE id='$otherId' AND display_name='Browser community fixture';"
        if ($LASTEXITCODE -ne 0) { throw 'Community member cleanup failed.' }
    }
    & $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c "DELETE FROM member_users WHERE id='$memberId' AND display_name='Browser test fixture';"
    if ($LASTEXITCODE -ne 0) { throw 'Fixture cleanup failed.' }
    Remove-Item -LiteralPath $fixturePath
    if (Test-Path -LiteralPath $browserStatePath) { Remove-Item -LiteralPath $browserStatePath }
    Write-Output 'Removed only the generated browser fixture member and its cascaded test sessions/links.'
    exit
}
if (Test-Path -LiteralPath $fixturePath) { throw 'Clean the prior browser fixture before creating another.' }
if ($IncludeIcon -and !$IncludeSite) { throw 'Icon fixture requires IncludeSite.' }
if ($IncludeSvgIcon -and (!$IncludeSite -or !$IncludeIcon)) { throw 'SVG icon fixture requires IncludeSite and IncludeIcon.' }
if ($IncludeHealth -and !$IncludeSite) { throw 'Health fixture requires IncludeSite.' }
if ($IncludeWorkerMonitor -and !$IncludeSite) { throw 'Worker monitor fixture requires IncludeSite.' }
if ($IncludeCommunity -and !$IncludeSite) { throw 'Community fixture requires IncludeSite.' }
if ($IncludeOwnComments -and !$IncludeCommunity) { throw 'Own comments fixture requires IncludeCommunity.' }
if ($IncludeModeration -and (!$IncludeCommunity -or !$IncludeSite)) { throw 'Moderation fixture requires IncludeSite and IncludeCommunity.' }
if ($IncludeCatalogAdmin -and !$IncludeCatalog) { throw 'Catalog administrator fixture requires IncludeCatalog.' }
$memberId = [Guid]::NewGuid().ToString()
$sessionId = [Guid]::NewGuid().ToString()
$accountId = [Guid]::NewGuid().ToString()
$tokenBytes = [byte[]]::new(32)
[Security.Cryptography.RandomNumberGenerator]::Fill($tokenBytes)
$token = [Convert]::ToHexString($tokenBytes).ToLowerInvariant()
$hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($token))).ToLowerInvariant()
$sql = "BEGIN; INSERT INTO member_users (id, display_name) VALUES ('$memberId', 'Browser test fixture'); INSERT INTO member_sessions (id,member_id,token_hash,expires_at) VALUES ('$sessionId','$memberId',decode('$hash','hex'),now()+interval '1 hour'); INSERT INTO member_linked_accounts (id,member_id,actor_url,handle,display_name,profile_url,provider,provider_origin,provider_subject_id,verified_at) VALUES ('$accountId','$memberId','https://browser-fixture.example/users/$memberId','@fixture_$($memberId.Replace('-',''))@browser-fixture.example','Browser test fixture','https://browser-fixture.example/@fixture','activitypub_post','https://browser-fixture.example','https://browser-fixture.example/users/$memberId',now()); COMMIT;"
if ($IncludeRegistration) {
    # The old-enough account is opt-in and synthetic; no public AP proof is bypassed in the application.
    $sql=$sql.Replace('COMMIT;', "UPDATE member_linked_accounts SET account_created_at=now()-interval '181 days',account_created_at_verified_at=now() WHERE id='$accountId' AND member_id='$memberId'; COMMIT;")
}
if ($IncludeSite) {
    $siteId = [Guid]::NewGuid().ToString()
    # Closed only to prevent the development worker from contacting this fake domain.
    $sql = $sql.Replace('COMMIT;', "INSERT INTO directory_sites(id,domain,name,is_closed) VALUES('$siteId','browser-$memberId.example.org','Browser owner fixture',true); INSERT INTO directory_site_details(site_id,owner_id,owner_method,rules,owner_comment) VALUES('$siteId','$memberId','dns','Fixture rules','Fixture owner comment'); COMMIT;")
}
if ($IncludeIcon) {
    # Valid 1x1 PNG fixture, not downloaded or copied from a product.
    $png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII='
    $mime='image/png'
    if($IncludeSvgIcon){
        # Synthetic SVG includes deliberate sentinels for direct-document CSP.
        $svg="<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64' width='64' height='64'><style>rect{fill:#deeee4}circle{fill:#247a51}</style><rect width='64' height='64' rx='12'/><circle cx='32' cy='32' r='17'/><script>window.__icon_executed=true;fetch('/__icon_probe')</script><image href='https://icon-fixture.invalid/blocked.png' width='1' height='1'/></svg>"
        $png=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($svg))
        $mime='image/svg+xml'
    }
    $sql = $sql.Replace('COMMIT;', "INSERT INTO directory_icons(site_id,mime,bytes) VALUES('$siteId','$mime',decode('$png','base64')); COMMIT;")
}
if ($IncludeHealth) {
    # Discrete synthetic checks. Site cleanup cascades to these rows; no crawler runs.
    $sql = $sql.Replace('COMMIT;', "INSERT INTO directory_health_checks(job_id,site_id,is_alive,response_time_ms,status_code,error,checked_at) SELECT gen_random_uuid(),'$siteId',n%8<>7,CASE WHEN n%8=7 OR n=61 THEN NULL WHEN n%8=6 THEN 2300 ELSE 180+n END,CASE WHEN n%8=7 THEN 503 ELSE 200 END,'private health fixture error','2026-09-14T00:00:00Z'::timestamptz+interval '1 minute'*n FROM generate_series(0,63) n; COMMIT;")
}
if ($IncludeCatalog) {
    $kindId = -([Convert]::ToInt64($memberId.Replace('-','').Substring(0,14),16)+1)
    $sql = $sql.Replace('COMMIT;', "INSERT INTO catalog_categories(id,name,label,emoji,display_order,inserted_at,updated_at) VALUES($kindId,'browser-kind-$memberId','Browser catalog kind','',0,now(),now()); COMMIT;")
}
if ($IncludeCommunity) {
    $otherId=[Guid]::NewGuid().ToString()
    $threadId=[Guid]::NewGuid().ToString()
    $ownCommentId=[Guid]::NewGuid().ToString()
    $sql=$sql.Replace('COMMIT;', "INSERT INTO member_users(id,display_name) VALUES('$otherId','Browser community fixture'); INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) VALUES('$threadId','$otherId','$siteId','Community fixture question',false,now() AT TIME ZONE 'UTC'-interval '5 minutes',now() AT TIME ZONE 'UTC'-interval '5 minutes'),('$ownCommentId','$memberId','$siteId','Own fixture comment',false,now() AT TIME ZONE 'UTC'-interval '4 minutes',now() AT TIME ZONE 'UTC'-interval '4 minutes'); INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),'$otherId','$siteId','$threadId','Fixture reply '||n,false,now() AT TIME ZONE 'UTC'-interval '3 minutes'+interval '1 second'*n,now() AT TIME ZONE 'UTC'-interval '3 minutes'+interval '1 second'*n FROM generate_series(1,4) n; COMMIT;")
}
if ($IncludeModeration) {
    # Synthetic role only; production grants go through the explicit audited CLI.
    $reportId=[Guid]::NewGuid().ToString()
    $sql=$sql.Replace('COMMIT;', "INSERT INTO member_admin_roles(member_id) VALUES('$memberId'); INSERT INTO community_reports(id,reporter_id,comment_id,reason,detail,status,inserted_at,updated_at) VALUES('$reportId','$memberId','$threadId','abuse','Private fixture report','pending',now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC'); INSERT INTO community_report_evidence(report_id,body,author_name,comment_updated_at) SELECT '$reportId',body,'Browser community fixture',updated_at FROM community_comments WHERE id='$threadId'; COMMIT;")
}
if ($IncludeOwnComments) {
    $sql=$sql.Replace('COMMIT;', "INSERT INTO community_comments(id,user_id,server_id,parent_id,body,is_deleted,inserted_at,updated_at) SELECT gen_random_uuid(),'$memberId','$siteId',CASE WHEN n=1 THEN '$threadId'::uuid END,CASE WHEN n=1 THEN repeat(chr(44032),2001) ELSE 'Own history '||n END,false,(now() AT TIME ZONE 'UTC')-interval '1 minute'*(n+5),now() AT TIME ZONE 'UTC' FROM generate_series(1,24) n; INSERT INTO community_comments(id,user_id,server_id,body,is_deleted,inserted_at,updated_at) VALUES(gen_random_uuid(),'$memberId','$siteId','Deleted own fixture secret',true,now() AT TIME ZONE 'UTC',now() AT TIME ZONE 'UTC'); UPDATE community_comments SET body='Own fixture comment <script>window.ownCommentExecuted=true</script>' WHERE id='$ownCommentId'; COMMIT;")
}
if (($IncludeCatalogAdmin -or $IncludeProfileBatch -or $IncludeWorkerMonitor) -and !$IncludeModeration) {
    $sql=$sql.Replace('COMMIT;', "INSERT INTO member_admin_roles(member_id) VALUES('$memberId'); COMMIT;")
}
$workerSiteId = $null
if ($IncludeWorkerMonitor) {
    $workerSiteId = [Guid]::NewGuid().ToString()
    # The closed primary fixture contributes stable dead/paused jobs. A held
    # lease prevents either local development worker from contacting the
    # second synthetic site while its latest-observation UI is inspected.
    # Clean this fixture promptly; this does not prove live crawler success.
    $sql=$sql.Replace('COMMIT;', "INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,last_error) VALUES(gen_random_uuid(),'$siteId',now()-interval '10 minutes','dead',3,'private worker raw exception sentinel'),(gen_random_uuid(),'$siteId',now()-interval '9 minutes','pending',0,NULL); INSERT INTO directory_sites(id,domain,name,is_closed,is_hidden) VALUES('$workerSiteId','worker-$memberId.example.org','Browser worker fixture',false,true); INSERT INTO directory_jobs(id,site_id,scheduled_at,state,attempts,lease_token,lease_until) VALUES(gen_random_uuid(),'$workerSiteId',now()-interval '5 minutes','running',1,gen_random_uuid(),now()+interval '2 hours'); INSERT INTO directory_observations(site_id,is_alive,status_code,health_error,checked_at,nodeinfo_error) VALUES('$workerSiteId',false,503,'private remote error sentinel',now(),'private nodeinfo error sentinel'); COMMIT;")
}
$profileMembers = @()
if ($IncludeProfileBatch) {
    # Empty-profile targets keep real worker progress visible long enough to
    # exercise cancellation in the browser; no remote identities are invented.
    foreach ($index in 1..30) {
        $extraId = [Guid]::NewGuid().ToString()
        $profileMembers += $extraId
        $sql = $sql.Replace('COMMIT;', "INSERT INTO member_users(id,display_name) VALUES('$extraId','Browser profile batch fixture'); COMMIT;")
    }
}
& $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -v ON_ERROR_STOP=1 -c $sql
if ($LASTEXITCODE -ne 0) { throw 'Browser fixture creation failed.' }
# Generated test secrets stay in the ignored local fixture file, never logs.
@{ member_id=$memberId; token=$token; site_id=$siteId; worker_site_id=$workerSiteId; catalog_kind_id=$kindId; community_member_id=$otherId; report_id=$reportId; moderation=[bool]$IncludeModeration; catalog_admin=[bool]$IncludeCatalogAdmin; profile_member_ids=$profileMembers } | ConvertTo-Json | Set-Content -LiteralPath $fixturePath -Encoding utf8
@{ cookies=@(@{name='fedkr_session';value=$token;domain='127.0.0.1';path='/';expires=-1;httpOnly=$true;secure=$false;sameSite='Lax'});origins=@() } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $browserStatePath -Encoding utf8
Write-Output 'Created isolated browser fixture (not proof of live ActivityPub interoperability).'
