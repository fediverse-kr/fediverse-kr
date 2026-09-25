# Read-only checks; no member fixture, credentials, or production target.
param([ValidateRange(1024,65535)][int]$Port=12239)
$ErrorActionPreference = 'Stop'
$base = "http://127.0.0.1:$Port"
$checks = 0
function Assert-Status([string]$path, [int]$expected) {
    $response = Invoke-WebRequest -Uri ($base + $path) -UseBasicParsing -SkipHttpErrorCheck -TimeoutSec 15
    if ($response.StatusCode -ne $expected) { throw "Unexpected HTTP status at $path (wanted $expected, got $($response.StatusCode))" }
    $script:checks++
}
$catalog = Invoke-RestMethod -Uri "$base/api/public/catalog" -TimeoutSec 15
Assert-Status '/healthz' 200
Assert-Status '/readyz' $(if ($catalog.preview) {503} else {200})
Assert-Status '/not-a-real-fedkr-route' 404
Assert-Status '/start/not-a-real-fedkr-topic' 404
Assert-Status '/software/not-a-real-fedkr-software' 404
Assert-Status '/servers/no-such-server.invalid' 404
Assert-Status '/api/public/servers?query=&software=&page=10001' 400
Assert-Status ('/api/public/servers?query=' + ('a' * 257) + '&software=&page=0') 400
Assert-Status ('/api/public/server/' + ('a' * 254)) 400
Assert-Status '/api/public/statistics' 200
Assert-Status '/api/public/servers?query=&software=&page=0' 200
Assert-Status '/api/public/software-search?filters=' 200
Assert-Status '/api/public/software-search?filters=page%3D0' 400
Assert-Status '/platforms?page=0' 400
Assert-Status '/api/public/comments?domain=light.example&page=0' $(if ($catalog.preview) {200} else {404})
Assert-Status '/api/member/comment-access?domain=light.example&ids=' $(if ($catalog.preview) {200} else {404})
Write-Output "Public HTTP checks passed: $checks. Preview: $($catalog.preview). No writes performed."
