#requires -Version 7.0
# Public synthetic catalog only: no members, sessions, network targets or real DB inputs.
param([ValidateSet('Seed','Clean')][string]$Mode='Seed')
$ErrorActionPreference='Stop'
$workspace=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$markerFile=Join-Path $workspace '.local/catalog-browser-fixture.json'
$localDir=Get-Item -LiteralPath (Split-Path -Parent $markerFile)
if (!$localDir.PSIsContainer -or ($localDir.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Private workspace fixture directory required.' }
function Sql([string]$query) {
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName='C:\Program Files\PostgreSQL\17\bin\psql.exe';$info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardInput=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $info.StandardInputEncoding=[Text.UTF8Encoding]::new($false);$info.StandardOutputEncoding=[Text.UTF8Encoding]::new($false)
    $info.Environment['PGCLIENTENCODING']='UTF8'
    foreach($arg in @('-X','-h','127.0.0.1','-p','16439','-U','fedkr_dev','-d','fedkr_dev','-v','ON_ERROR_STOP=1','-At')) {$info.ArgumentList.Add($arg)}
    $proc=[Diagnostics.Process]::Start($info)
    try {
        $out=$proc.StandardOutput.ReadToEndAsync();$err=$proc.StandardError.ReadToEndAsync()
        $proc.StandardInput.WriteLine($query);$proc.StandardInput.Close()
        if (!$proc.WaitForExit(30000)) {$proc.Kill($true);throw 'Synthetic SQL deadline exceeded.'}
        [void]$err.GetAwaiter().GetResult()
        if($proc.ExitCode -ne 0){throw 'Synthetic catalog operation failed; fixture marker retained.'}
        return $out.GetAwaiter().GetResult().Trim()
    } finally {if(!$proc.HasExited){$proc.Kill($true);$proc.WaitForExit()};$proc.Dispose()}
}
if ([IO.Path]::GetFullPath((Sql 'SHOW data_directory')) -ne (Join-Path $workspace '.local/postgres')) {throw 'Only the workspace synthetic cluster is allowed.'}
if($Mode -eq 'Seed') {
    if(Test-Path -LiteralPath $markerFile){throw 'Clean the recorded fixture first.'}
    $id=[Guid]::NewGuid().ToString('N')
    @{id=$id} | ConvertTo-Json | Set-Content -LiteralPath $markerFile -Encoding utf8
} else {
    $item=Get-Item -LiteralPath $markerFile
    if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked fixture marker refused.'}
    $id=[string](Get-Content -LiteralPath $markerFile -Raw | ConvertFrom-Json).id
    if($id -cnotmatch '^[a-f0-9]{32}$'){throw 'Invalid fixture marker.'}
}
$prefix="browse-$id"
if($Mode -eq 'Clean') {
    # Exact generated names only. No broad deletion or production tables.
    [void](Sql "BEGIN; DELETE FROM catalog_software WHERE name IN (SELECT '$prefix-tool-'||lpad(n::text,2,'0') FROM generate_series(1,27) n); DELETE FROM catalog_categories WHERE name IN (SELECT '$prefix-kind-'||lpad(n::text,2,'0') FROM generate_series(1,14) n); COMMIT;")
    Remove-Item -LiteralPath $markerFile
    Write-Output 'Removed only the 27 synthetic software and 14 category fixtures; reproducible with Seed.'
    exit
}
[void](Sql "BEGIN;
INSERT INTO catalog_categories(id,name,label,emoji,display_order,inserted_at,updated_at)
 SELECT -abs(hashtextextended('$prefix'||n::text,1)), '$prefix-kind-'||lpad(n::text,2,'0'), '검토 종류 '||lpad(n::text,2,'0'),'◇',-100000+n,now(),now() FROM generate_series(1,14) n;
INSERT INTO catalog_software(id,name,display_name,description,categories,is_featured,display_order,inserted_at,updated_at)
 SELECT -abs(hashtextextended('$prefix'||n::text,2)), '$prefix-tool-'||lpad(n::text,2,'0'),
 CASE WHEN n=27 THEN '검토 사진%_+ 도구' ELSE '검토 도구 '||lpad(n::text,2,'0') END,
 '스크롤과 검색을 검토하기 위한 가상 소프트웨어입니다. 실제 서비스가 아닙니다.',
 ARRAY['$prefix-kind-'||CASE WHEN n%2=0 THEN '14' ELSE '01' END],false,0,now(),now() FROM generate_series(1,27) n;
COMMIT;")
Write-Output 'Created 27 synthetic software and 14 categories. No real accounts or remote requests.'
