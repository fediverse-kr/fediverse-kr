#requires -Version 7.0
# Manual integration check, not an offline test-suite member. Only public reads
# go to the remote site. A synthetic local member calls preview, never create.
param(
    [ValidateSet('mastodon.social','misskey.io')][string]$Domain = 'mastodon.social',
    [switch]$AllowPublicNetworkRead
)
$ErrorActionPreference='Stop'
if (!$AllowPublicNetworkRead) { throw 'Explicit -AllowPublicNetworkRead is required; no request or fixture was created.' }
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$fixture=Join-Path $workspace '.local/browser-fixture.json'
$mediaFixture=Join-Path $workspace '.local/media-browser-fixture.json'
$setup=Join-Path $PSScriptRoot 'member-browser-fixture.ps1'
if ((Test-Path -LiteralPath $fixture) -or (Test-Path -LiteralPath $mediaFixture)) {
    throw 'Existing local browser fixture must remain untouched.'
}
$origin='http://127.0.0.1:12239'
$handler=[Net.Http.HttpClientHandler]::new()
$handler.AllowAutoRedirect=$false
$handler.UseProxy=$false
$http=[Net.Http.HttpClient]::new($handler)
$http.Timeout=[TimeSpan]::FromSeconds(40)
$created=$false
$fixtureId=$null
$token=$null
$report=$null
function Request([string]$path,[string]$body='') {
    $method=if($body){[Net.Http.HttpMethod]::Post}else{[Net.Http.HttpMethod]::Get}
    $message=[Net.Http.HttpRequestMessage]::new($method,$origin+$path)
    if($token){[void]$message.Headers.TryAddWithoutValidation('Cookie',"fedkr_session=$token")}
    if($body){
        [void]$message.Headers.TryAddWithoutValidation('Origin',$origin)
        $message.Content=[Net.Http.StringContent]::new($body,[Text.Encoding]::UTF8,'application/json')
    }
    try {
        $response=$http.SendAsync($message).GetAwaiter().GetResult()
        try { return @{Status=[int]$response.StatusCode; Text=$response.Content.ReadAsStringAsync().GetAwaiter().GetResult()} }
        finally {$response.Dispose()}
    } finally {$message.Dispose()}
}
try {
    if((Request '/readyz').Status -ne 200){throw 'Preview not ready'}
    # The existing helper verifies the exact workspace PG cluster first.
    $null=& $setup -Mode Seed -IncludeRegistration
    $created=$true
    $identity=Get-Content -Raw -LiteralPath $fixture | ConvertFrom-Json
    $fixtureId=[Guid]::Parse($identity.member_id).ToString()
    $token=[string]$identity.token
    if($token -cnotmatch '^[a-f0-9]{64}$'){throw 'Invalid fixture'}
    if((Request '/api/member/sites/registration').Status -ne 200){throw 'Fixture not eligible'}
    $before=Request "/api/public/server/$Domain"
    if($before.Status -ne 200 -or ($before.Text|ConvertFrom-Json).site){throw 'Use an unregistered public target'}
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $result=Request '/api/member/sites/registration/preview' (@{domain=$Domain}|ConvertTo-Json -Compress)
    $clock.Stop()
    $observed=if($result.Status -eq 200){$result.Text|ConvertFrom-Json}else{$null}
    $after=Request "/api/public/server/$Domain"
    if($after.Status -ne 200 -or ($after.Text|ConvertFrom-Json).site){throw 'Preview persisted a site'}
    $expected=if($Domain -eq 'mastodon.social'){'mastodon'}else{'misskey'}
    # Only fixed categories/booleans are emitted; no member IDs, credentials,
    # remote descriptions, user counts, database rows or raw HTTP error bodies.
    $report=[pscustomobject]@{
        target=$Domain
        http_status=$result.Status
        preview_verified=($result.Status -eq 200 -and $observed.domain -eq $Domain -and $observed.software -eq $expected)
        software_matches=($observed.software -eq $expected)
        user_count_present=($null -ne $observed.users)
        registration_flag_present=($null -ne $observed.registration_open)
        elapsed_seconds=[Math]::Round($clock.Elapsed.TotalSeconds,2)
        created_remote_content=$false
        registered_local_site=$false
        signed_activitypub_verified=$false
    }
} catch {
    throw 'Live preview check could not finish; raw responses and fixture details withheld.'
} finally {
    $http.Dispose()
    if($created){
        $current=Get-Content -Raw -LiteralPath $fixture | ConvertFrom-Json
        if($current.member_id -ne $fixtureId -or $current.token -ne $token){
            throw 'Fixture changed; retained for inspection instead of cleaning another run.'
        }
        $null=& $setup -Mode Clean
    }
}
$report | ConvertTo-Json -Compress
if(!$report.preview_verified){throw 'The live public preview did not pass. No local server registration was performed.'}
