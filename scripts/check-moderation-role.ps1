# Explicit synthetic fixture only. Never accepts a real member or DB URL.
param([Parameter(Mandatory)][string]$ServerBinary, [ValidateRange(1024,65535)][int]$Port=12239)
$ErrorActionPreference='Stop'
$projectPath=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$binaryPath=(Resolve-Path -LiteralPath $ServerBinary).Path
if (!$binaryPath.StartsWith((Join-Path $projectPath 'target')+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {throw 'Only this workspace test binary is allowed.'}
$f=Get-Content -Raw -LiteralPath (Join-Path $projectPath '.local/browser-fixture.json') | ConvertFrom-Json
if (!$f.moderation) {throw 'Moderation fixture required.'}
$memberId=[Guid]::Parse($f.member_id).ToString()
$psqlPath='C:\Program Files\PostgreSQL\17\bin\psql.exe'
$cluster=(& $psqlPath -h 127.0.0.1 -p 16439 -U fedkr_dev -d fedkr_dev -Atc 'SHOW data_directory').Trim()
if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($cluster) -ne (Join-Path $projectPath '.local/postgres')) {throw 'Wrong fixture cluster.'}
$base="http://127.0.0.1:$Port"
$headers=@{Cookie="fedkr_session=$($f.token)";Origin=$base}
$me=Invoke-RestMethod "$base/api/member/session" -Headers $headers
if ($me.member.id -ne $memberId -or $me.member.display_name -ne 'Browser test fixture') {throw 'Synthetic authenticated member required.'}
$env:FEDKR_DATABASE_URL='postgresql://fedkr_dev@127.0.0.1:16439/fedkr_dev'
$env:FEDKR_PUBLIC_ORIGIN=$base
$list=Invoke-RestMethod "$base/api/member/moderation/reports?status=all&page=0" -Headers $headers
$reportId=[Guid]::Parse(($list.reports | Where-Object {$_.domain -eq "browser-$memberId.example.org"} | Select-Object -First 1).id).ToString()
$d=Invoke-RestMethod "$base/api/member/moderation/report?id=$reportId" -Headers $headers
$siteId=[Guid]::Parse($f.site_id).ToString()
$site=Invoke-RestMethod "$base/api/member/moderation/site?id=$siteId" -Headers $headers
$sitePayload=@{request=@{id=$siteId;revision=$site.revision;action=@{kind='hidden';value=$true};note='fixture forbidden attempt'}} | ConvertTo-Json -Depth 5
$payload=@{request=@{report=$reportId;revision=$d.revision;comment_revision=$d.comment.revision;author_id=$d.comment.author_id;author_banned=$d.comment.author_banned;author_revision=$d.comment.author_revision;action='resolve';note='fixture forbidden attempt'}} | ConvertTo-Json -Depth 5
& $binaryPath --admin revoke $memberId
if ($LASTEXITCODE -ne 0) {throw 'CLI revoke failed.'}
try {
    if (Invoke-RestMethod "$base/api/member/moderation/access" -Headers $headers) {throw 'Revoked member still has access.'}
    foreach ($path in @('/api/member/moderation/reports?status=all&page=0',"/api/member/moderation/report?id=$reportId","/api/member/moderation/events?id=$reportId&page=0",'/api/member/moderation/sites?query=&status=all&sort=domain&page=0',"/api/member/moderation/site?id=$siteId","/api/member/moderation/site/history?id=$siteId&page=0",'/api/member/moderation/workers')) {
        $r=Invoke-WebRequest "$base$path" -Headers $headers -SkipHttpErrorCheck
        if ($r.StatusCode -ne 403 -or $r.Content.Contains('Private fixture report')) {throw 'Private read was not denied.'}
    }
    $r=Invoke-WebRequest "$base/api/member/moderation/act" -Method Post -Headers $headers -ContentType 'application/json' -Body $payload -SkipHttpErrorCheck
    if ($r.StatusCode -ne 403) {throw 'Non-admin write was not denied.'}
    $r=Invoke-WebRequest "$base/account/moderation/$reportId" -Headers $headers -SkipHttpErrorCheck
    if ($r.Content.Contains('Private fixture report')) {throw 'Non-admin SSR leaked report data.'}
    if (!(($r.Headers['Cache-Control'] -join ',').Contains('no-store'))) {throw 'Non-admin SSR lacks no-store.'}
    $r=Invoke-WebRequest "$base/api/member/moderation/site/act" -Method Post -Headers $headers -ContentType 'application/json' -Body $sitePayload -SkipHttpErrorCheck
    if ($r.StatusCode -ne 403) {throw 'Non-admin site write was not denied.'}
    $r=Invoke-WebRequest "$base/account/moderation/sites/$siteId" -Headers $headers -SkipHttpErrorCheck
    if ($r.Content.Contains("browser-$memberId.example.org") -or $r.Content.Contains('Browser owner fixture')) {throw 'Non-admin SSR leaked site data.'}
    if (!(($r.Headers['Cache-Control'] -join ',').Contains('no-store'))) {throw 'Non-admin site SSR lacks no-store.'}
    $r=Invoke-WebRequest "$base/account/moderation/workers" -Headers $headers -SkipHttpErrorCheck
    if ($r.Content.Contains("browser-$memberId.example.org") -or $r.Content.Contains("worker-$memberId.example.org") -or !(($r.Headers['Cache-Control'] -join ',').Contains('no-store'))) {throw 'Non-admin worker SSR disclosed private data or lacks no-store.'}
    if ($f.catalog_admin) {
        $name="browser-$memberId"
        $kind="browser-kind-$memberId"
        foreach ($path in @('/api/member/moderation/software?query=&locked=false&page=0',"/api/member/moderation/software/item?name=$name","/api/member/moderation/software/editable?name=$name",'/api/member/moderation/categories?query=&page=0',"/api/member/moderation/category?name=$kind","/api/member/moderation/catalog/history?name=$name&category=false&page=0","/account/moderation/catalog/software/$name","/account/moderation/catalog/software/$name/edit","/account/moderation/catalog/category/$kind",'/account/moderation/catalog/categories/new')) {
            $r=Invoke-WebRequest "$base$path" -Headers $headers -SkipHttpErrorCheck
            if ($r.StatusCode -ne 403 -or $r.Content.Contains('관리자만 읽는 테스트 사유') -or !(($r.Headers['Cache-Control'] -join ',').Contains('no-store'))) {throw 'Non-admin catalog read/SSR not denied.'}
        }
        $requests=@(
            @{path='/api/member/moderation/software/act';data=@{request=@{name=$name;revision=1;action=@{kind='locked';value=$false};note='forbidden attempt'}}},
            @{path='/api/member/moderation/software/save';data=@{name=$name;revision=1;edit=@{display_name='Forbidden';family='';description='';categories=@($kind);features=@();website_url='';tech_stack=''};summary='forbidden attempt'}},
            @{path='/api/member/moderation/category/save';data=@{request=@{name=$kind;revision=0;edit=@{label='Forbidden';emoji='';display_order=0};note='forbidden attempt'}}}
        )
        foreach($request in $requests){
            $r=Invoke-WebRequest "$base$($request.path)" -Method Post -Headers $headers -ContentType 'application/json' -Body ($request.data|ConvertTo-Json -Depth 8) -SkipHttpErrorCheck
            if($r.StatusCode -ne 403){throw 'Non-admin catalog write not denied.'}
        }
    }
} finally {
    & $binaryPath --admin grant $memberId
    if ($LASTEXITCODE -ne 0) {throw 'CLI regrant failed.'}
}
& $binaryPath --admin grant $memberId
if ($LASTEXITCODE -ne 0) {throw 'Idempotent grant failed.'}
if (!(Invoke-RestMethod "$base/api/member/moderation/access" -Headers $headers)) {throw 'Explicit role grant was not visible.'}
Write-Output 'Role CLI + non-admin HTTP/SSR checks passed; only synthetic fixture member changed.'
