# Small loopback-only negative requests, including real chunked transfer.
$ErrorActionPreference = 'Stop'
$base = 'http://127.0.0.1:12239'
$oversized = 'x' * 9000
$chunkedStatus = $oversized | curl.exe --silent --output NUL --write-out '%{http_code}' --http1.1 -H "Origin: $base" -H 'Content-Type: application/json' -H 'Transfer-Encoding: chunked' --data-binary '@-' "$base/api/member/logout"
if ($LASTEXITCODE -ne 0 -or $chunkedStatus -ne '413') { throw "Chunked body limit failed: HTTP $chunkedStatus" }
$originStatus = $oversized | curl.exe --silent --output NUL --write-out '%{http_code}' --http1.1 -H 'Origin: https://evil.example' -H 'Content-Type: application/json' -H 'Transfer-Encoding: chunked' --data-binary '@-' "$base/api/member/logout"
if ($LASTEXITCODE -ne 0 -or $originStatus -ne '403') { throw "Origin-before-body check failed: HTTP $originStatus" }
$alive = Invoke-WebRequest "$base/api/member/session" -TimeoutSec 10
if ($alive.StatusCode -ne 200 -or $alive.Headers['Cache-Control'] -notmatch 'no-store') { throw 'Session endpoint did not remain healthy/private.' }
Write-Output 'PASS: chunked body limit 413, Origin before body 403, healthy no-store session 200.'
