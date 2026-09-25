[CmdletBinding()]
param(
    [ValidateSet('Extract', 'Check')]
    [string] $Mode = 'Extract',
    [string] $ArchivePath,
    [SecureString] $Password,
    [string] $Batch
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ManifestSchema = 2
$RootNamePrefix = 'archive-'
$FilesName = 'files'
$ManifestName = 'manifest.json'
$SharpZipLibHash = '32ea2d0ce3512e74f1c7ad82591fe67e6b8939d76a8a4ff9c93ead030131e71c'
$MaximumArchiveBytes = [int64]2GB
$MaximumEntries = 256
$MaximumEntryBytes = [int64]2GB
$MaximumTotalBytes = [int64]4GB
$MaximumCompressionRatio = 100L
$MaximumPathLength = 240
$MaximumPathDepth = 8
$ExtractionBudget = [TimeSpan]::FromMinutes(30)
$ArchivePathWasSupplied = $PSBoundParameters.ContainsKey('ArchivePath')
$PasswordWasSupplied = $PSBoundParameters.ContainsKey('Password')
$BatchWasSupplied = $PSBoundParameters.ContainsKey('Batch')

. (Join-Path $PSScriptRoot 'private-import-common.ps1')

function Assert-ArchiveRuntime {
    if ($PSVersionTable.PSVersion -lt [Version]'7.4') { Fail 'PowerShell 7.4 or later is required.' }
    if ([Environment]::Version.Major -lt 8) { Fail '.NET 8 or later is required.' }
}

function Initialize-ArchiveLibraries {
    $sharpPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\\.local\\tools\\sharpziplib-1.4.2\\ICSharpCode.SharpZipLib.dll'))
    $streamPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'private-archive-stream.cs'))
    if (-not (Test-Path -LiteralPath $sharpPath -PathType Leaf) -or -not (Test-Path -LiteralPath $streamPath -PathType Leaf)) {
        Fail 'Required private archive libraries are unavailable.'
    }
    $actualHash = ([Security.Cryptography.SHA256]::HashData([IO.File]::ReadAllBytes($sharpPath)) | ForEach-Object { $_.ToString('x2') }) -join ''
    if ($actualHash -ne $SharpZipLibHash) { Fail 'Required private archive libraries are invalid.' }

    $loadedSharp = 'ICSharpCode.SharpZipLib.Zip.ZipFile' -as [type]
    if ($null -eq $loadedSharp) {
        Add-Type -Path $sharpPath
    } elseif ([IO.Path]::GetFullPath($loadedSharp.Assembly.Location) -ne $sharpPath) {
        Fail 'A different archive library is already loaded.'
    }

    # This script is designed for a fresh PowerShell process. Refuse a
    # preexisting helper rather than accepting a type from an unknown source.
    if ($null -ne ('Fedkr.PrivateBackup.BoundedWriteStream' -as [type])) {
        Fail 'A private archive stream helper is already loaded.'
    }
    Add-Type -Path $streamPath
}

function Get-PrivateSha256([string] $Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        ([Security.Cryptography.SHA256]::HashData($stream) | ForEach-Object { $_.ToString('x2') }) -join ''
    } finally {
        $stream.Dispose()
    }
}

function Get-ArchiveRoot([pscustomobject] $Context, [string] $RequestedBatch) {
    Assert-Batch $RequestedBatch
    $root = [IO.Path]::GetFullPath((Join-Path $Context.Base ($RootNamePrefix + $RequestedBatch))).TrimEnd('\')
    $baseWithSeparator = $Context.Base.TrimEnd('\') + '\'
    if (-not $root.StartsWith($baseWithSeparator, [StringComparison]::OrdinalIgnoreCase)) { Fail 'Archive root is invalid.' }
    $root
}

function ConvertTo-ArchiveRelativePath([string] $Value, [switch] $Directory) {
    if ([string]::IsNullOrEmpty($Value) -or $Value.Length -gt $MaximumPathLength) { Fail 'Archive entry path is invalid.' }
    if (-not [string]::Equals($Value, $Value.Normalize([Text.NormalizationForm]::FormC), [StringComparison]::Ordinal)) { Fail 'Archive entry path is invalid.' }
    if ($Value.IndexOfAny([char[]]@('\', ':', '<', '>', '"', '|', '?', '*')) -ge 0 -or $Value -match '[\p{Cc}]' -or $Value.StartsWith('/')) { Fail 'Archive entry path is invalid.' }
    $relative = $Value
    if ($Directory) {
        if (-not $relative.EndsWith('/')) { Fail 'Archive entry path is invalid.' }
        $relative = $relative.Substring(0, $relative.Length - 1)
    } elseif ($relative.EndsWith('/')) {
        Fail 'Archive entry path is invalid.'
    }
    if ([string]::IsNullOrEmpty($relative)) { Fail 'Archive entry path is invalid.' }
    $segments = @($relative.Split('/'))
    if ($segments.Count -gt $MaximumPathDepth -or $segments.Count -eq 0) { Fail 'Archive entry path is invalid.' }
    foreach ($segment in $segments) {
        if ([string]::IsNullOrEmpty($segment) -or $segment -eq '.' -or $segment -eq '..' -or $segment.EndsWith('.') -or $segment.EndsWith(' ')) { Fail 'Archive entry path is invalid.' }
        $device = $segment.Split('.')[0].TrimEnd('.', ' ')
        if ($device -match '^(?i:con|prn|aux|nul|com[1-9]|lpt[1-9])$') { Fail 'Archive entry path is invalid.' }
    }
    [pscustomobject]@{ Path = $relative; Segments = $segments }
}

function Add-ArchivePath {
    param(
        [pscustomobject] $PathInfo,
        [bool] $Directory,
        [System.Collections.Generic.HashSet[string]] $Files,
        [System.Collections.Generic.HashSet[string]] $Directories,
        [System.Collections.Generic.HashSet[string]] $DeclaredDirectories
    )
    $parts = $PathInfo.Segments
    for ($index = 1; $index -lt $parts.Count; $index++) {
        $parent = ($parts[0..($index - 1)] -join '/')
        if ($Files.Contains($parent)) { Fail 'Archive entry paths conflict.' }
        [void]$Directories.Add($parent)
    }
    if ($Directory) {
        if ($Files.Contains($PathInfo.Path) -or $null -eq $DeclaredDirectories -or -not $DeclaredDirectories.Add($PathInfo.Path)) { Fail 'Archive entry paths conflict.' }
        [void]$Directories.Add($PathInfo.Path)
    } else {
        if ($Directories.Contains($PathInfo.Path) -or -not $Files.Add($PathInfo.Path)) { Fail 'Archive entry paths conflict.' }
    }
}

function Assert-EntryMetadata([object] $Entry) {
    if (-not $Entry.CanDecompress -or ($Entry.IsDirectory -and $Entry.IsFile) -or (-not $Entry.IsDirectory -and -not $Entry.IsFile)) {
        Fail 'Archive entry metadata is unsupported.'
    }
    if ($Entry.CompressionMethod -ne [ICSharpCode.SharpZipLib.Zip.CompressionMethod]::Stored -and $Entry.CompressionMethod -ne [ICSharpCode.SharpZipLib.Zip.CompressionMethod]::Deflated) {
        Fail 'Archive entry compression is unsupported.'
    }
    if ($Entry.Size -lt 0 -or $Entry.Size -gt $MaximumEntryBytes -or $Entry.CompressedSize -lt 0) { Fail 'Archive entry size is invalid.' }
    if ($Entry.Size -gt 0 -and ($Entry.CompressedSize -eq 0 -or $Entry.Size -gt ($Entry.CompressedSize * $MaximumCompressionRatio))) { Fail 'Archive entry compression ratio is invalid.' }
    if ($Entry.ExternalFileAttributes -ne -1) {
        $attributes = ([int64]$Entry.ExternalFileAttributes -band 0xffffffffL)
        if (($attributes -band [int64][IO.FileAttributes]::ReparsePoint) -ne 0) { Fail 'Archive entry metadata is unsupported.' }
        $unixType = (($attributes -shr 16) -band 0xF000)
        if ($unixType -ne 0 -and $unixType -ne 0x8000 -and $unixType -ne 0x4000) { Fail 'Archive entry metadata is unsupported.' }
    }
    if (-not $Entry.IsDirectory -and -not $Entry.IsCrypted) { Fail 'Archive files must be encrypted.' }
}

function Get-ArchivePlans([object] $Archive) {
    if ($Archive.IsEmbeddedArchive -or $Archive.IsNewArchive) { Fail 'Archive metadata is unsupported.' }
    $files = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $directories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $declaredDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $plans = [System.Collections.Generic.List[object]]::new()
    $directoryPlans = [System.Collections.Generic.List[string]]::new()
    $total = 0L
    foreach ($entry in @($Archive)) {
        if ($plans.Count -ge $MaximumEntries) { Fail 'Archive has too many entries.' }
        Assert-EntryMetadata $entry
        $pathInfo = ConvertTo-ArchiveRelativePath ([string]$entry.Name) -Directory:$entry.IsDirectory
        Add-ArchivePath $pathInfo $entry.IsDirectory $files $directories $declaredDirectories
        if ($entry.Size -gt ($MaximumTotalBytes - $total)) { Fail 'Archive total size is invalid.' }
        $total += [int64]$entry.Size
        $plans.Add([pscustomobject]@{ Entry = $entry; Path = $pathInfo.Path; Segments = $pathInfo.Segments; Directory = [bool]$entry.IsDirectory; Size = [int64]$entry.Size })
        if ($entry.IsDirectory) { $directoryPlans.Add($pathInfo.Path) }
    }
    [pscustomobject]@{ Plans = $plans.ToArray(); Directories = $directoryPlans.ToArray(); Entries = $plans.Count; Bytes = $total }
}

function Write-Manifest([string] $Path, [object] $Manifest, [string] $Sid, [switch] $Create) {
    $payload = [Text.UTF8Encoding]::new($false).GetBytes((($Manifest | ConvertTo-Json -Depth 8) + "`n"))
    $mode = if ($Create) { [IO.FileMode]::CreateNew } else { [IO.FileMode]::Truncate }
    $stream = [IO.File]::Open($Path, $mode, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $stream.Write($payload, 0, $payload.Length)
        $stream.Flush($true)
    } finally {
        $stream.Dispose()
    }
    if ($Create) { Set-PrivateFileAcl $Path $Sid }
}

function New-SafeParentDirectories([string] $FilesRoot, [string[]] $Segments, [string] $Sid) {
    $current = $FilesRoot
    for ($index = 0; $index -lt ($Segments.Count - 1); $index++) {
        $current = Join-Path $current $Segments[$index]
        [IO.Directory]::CreateDirectory($current) | Out-Null
        Set-PrivateDirectoryAcl $current $Sid
        Assert-NoReparse $current
    }
    $current
}

function Get-SafeExtractedFilePath([string] $FilesRoot, [string] $Relative) {
    $info = ConvertTo-ArchiveRelativePath $Relative
    $path = [IO.Path]::GetFullPath((Join-Path $FilesRoot ($info.Segments -join '\')))
    $prefix = $FilesRoot.TrimEnd('\') + '\'
    if (-not $path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { Fail 'Archive entry path is invalid.' }
    [pscustomobject]@{ Path = $path; Info = $info }
}

function Test-ArchiveFiles([string] $Root, [object] $Manifest, [string] $Sid) {
    Assert-NoReparse $Root
    Assert-PrivateAcl $Root $Sid
    $filesRoot = Join-Path $Root $FilesName
    if (-not (Test-Path -LiteralPath $filesRoot -PathType Container)) { Fail 'Archive output is invalid.' }
    $topLevel = @([IO.Directory]::GetFileSystemEntries($Root) | ForEach-Object { [IO.Path]::GetFileName($_) })
    if ($topLevel.Count -ne 2 -or $topLevel -notcontains $FilesName -or $topLevel -notcontains $ManifestName) { Fail 'Archive output is invalid.' }

    $expectedFiles = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $expectedDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $declaredDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $bytes = 0L
    $records = [object[]]@($Manifest.entries)
    $directoryRecords = [object[]]@($Manifest.directories)
    if ($records.Count + $directoryRecords.Count -gt $MaximumEntries) { Fail 'Archive manifest is invalid.' }
    foreach ($directoryRecord in $directoryRecords) {
        if ($directoryRecord -isnot [string]) { Fail 'Archive manifest is invalid.' }
        $directoryInfo = ConvertTo-ArchiveRelativePath ($directoryRecord + '/') -Directory
        Add-ArchivePath $directoryInfo $true $expectedFiles $expectedDirectories $declaredDirectories
    }
    foreach ($record in $records) {
        $required = @('path', 'size', 'sha256')
        if ($null -eq $record -or @($record.PSObject.Properties.Name | Where-Object { $_ -notin $required }).Count -ne 0 -or @($required | Where-Object { $record.PSObject.Properties.Name -notcontains $_ }).Count -ne 0) { Fail 'Archive manifest is invalid.' }
        $size = 0L
        if (-not [int64]::TryParse([string]$record.size, [ref]$size) -or $size -lt 0 -or $size -gt $MaximumEntryBytes -or [string]$record.sha256 -notmatch '^[0-9a-f]{64}$') { Fail 'Archive manifest is invalid.' }
        if ($size -gt ($MaximumTotalBytes - $bytes)) { Fail 'Archive manifest is invalid.' }
        $bytes += $size
        $file = Get-SafeExtractedFilePath $filesRoot ([string]$record.path)
        Add-ArchivePath $file.Info $false $expectedFiles $expectedDirectories $declaredDirectories
        if (-not (Test-Path -LiteralPath $file.Path -PathType Leaf)) { Fail 'Archive output is incomplete.' }
        $item = Get-Item -LiteralPath $file.Path -Force -ErrorAction Stop
        if ($item.Length -ne $size -or (Get-PrivateSha256 $file.Path) -ne [string]$record.sha256) { Fail 'Archive output is invalid.' }
    }

    $actualFiles = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $actualDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($item in @(Get-ChildItem -LiteralPath $filesRoot -Force -Recurse -ErrorAction Stop)) {
        $relative = $item.FullName.Substring($filesRoot.TrimEnd('\').Length).TrimStart('\') -replace '\\', '/'
        $info = ConvertTo-ArchiveRelativePath $relative
        if ($info.Path -ne $relative) { Fail 'Archive output is invalid.' }
        if ($item -is [IO.DirectoryInfo]) { [void]$actualDirectories.Add($info.Path) } else { [void]$actualFiles.Add($info.Path) }
    }
    if (-not $actualFiles.SetEquals($expectedFiles) -or -not $actualDirectories.SetEquals($expectedDirectories)) { Fail 'Archive output is invalid.' }
    [pscustomobject]@{ Files = $records.Count; Directories = $directoryRecords.Count; Bytes = $bytes }
}

function Extract-PrivateArchive([pscustomobject] $Context) {
    if ($script:BatchWasSupplied) { Fail 'Batch is not accepted for Extract.' }
    if ([string]::IsNullOrWhiteSpace($ArchivePath) -or -not [IO.Path]::IsPathFullyQualified($ArchivePath)) { Fail 'Archive path must be absolute.' }
    $sourcePath = [IO.Path]::GetFullPath($ArchivePath)
    if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) { Fail 'Archive input is unavailable.' }
    $sourceItem = Get-Item -LiteralPath $sourcePath -Force -ErrorAction Stop
    if (($sourceItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $sourceItem.Length -gt $MaximumArchiveBytes) { Fail 'Archive input is invalid.' }
    if ($null -eq $Password) { $Password = Read-Host -Prompt 'Archive password' -AsSecureString }
    if ($null -eq $Password) { Fail 'Archive password is unavailable.' }

    $batchValue = [Guid]::NewGuid().ToString('N')
    $root = Get-ArchiveRoot $Context $batchValue
    $filesRoot = Join-Path $root $FilesName
    $manifestPath = Join-Path $root $ManifestName
    $rootCreated = $false
    $manifest = $null
    $stream = $null
    $archive = $null
    $plain = $null
    $bstr = [IntPtr]::Zero
    try {
        $stream = [IO.File]::Open($sourcePath, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        if ($stream.Length -gt $MaximumArchiveBytes) { Fail 'Archive input is invalid.' }
        $sourceHash = ([Security.Cryptography.SHA256]::HashData($stream) | ForEach-Object { $_.ToString('x2') }) -join ''
        $stream.Position = 0
        $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($Password)
        $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
        $archive = [ICSharpCode.SharpZipLib.Zip.ZipFile]::new($stream, $true)
        $archive.Password = $plain
        $plan = Get-ArchivePlans $archive

        Assert-NoReparse $Context.Base
        Set-PrivateDirectoryAcl $Context.Base $Context.Sid
        Assert-NoReparse $Context.Base
        Assert-PrivateAcl $Context.Base $Context.Sid -Shallow
        if (Test-Path -LiteralPath $root) { Fail 'Archive output already exists.' }
        [IO.Directory]::CreateDirectory($root) | Out-Null
        $rootCreated = $true
        Set-PrivateDirectoryAcl $root $Context.Sid
        [IO.Directory]::CreateDirectory($filesRoot) | Out-Null
        Set-PrivateDirectoryAcl $filesRoot $Context.Sid
        Assert-NoReparse $root
        Assert-PrivateAcl $root $Context.Sid

        $manifest = [ordered]@{
            schema = $ManifestSchema
            batch = $batchValue
            state = 'extracting'
            createdUtc = [DateTime]::UtcNow.ToString('O')
            sourceArchiveSha256 = $sourceHash
            directories = $plan.Directories
            entries = @()
            counts = [ordered]@{ entries = $plan.Entries; directories = $plan.Directories.Count; files = 0; bytes = $plan.Bytes }
        }
        Write-Manifest $manifestPath $manifest $Context.Sid -Create

        $clock = [Diagnostics.Stopwatch]::StartNew()
        $written = [System.Collections.Generic.List[object]]::new()
        foreach ($directoryPath in $plan.Directories) {
            $targetDirectory = Get-SafeExtractedFilePath $filesRoot $directoryPath
            [void](New-SafeParentDirectories $filesRoot $targetDirectory.Info.Segments $Context.Sid)
            [IO.Directory]::CreateDirectory($targetDirectory.Path) | Out-Null
            Set-PrivateDirectoryAcl $targetDirectory.Path $Context.Sid
            Assert-NoReparse $targetDirectory.Path
        }
        foreach ($entryPlan in $plan.Plans) {
            if ($clock.Elapsed -gt $ExtractionBudget) { Fail 'Archive extraction exceeded its resource limit.' }
            if ($entryPlan.Directory) { continue }
            $target = Get-SafeExtractedFilePath $filesRoot $entryPlan.Path
            [void](New-SafeParentDirectories $filesRoot $target.Info.Segments $Context.Sid)
            $destination = $null
            $bounded = $null
            $entryStream = $null
            try {
                $destination = [IO.File]::Open($target.Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
                $bounded = [Fedkr.PrivateBackup.BoundedWriteStream]::new($destination, $entryPlan.Size, $clock, $ExtractionBudget)
                $entryStream = $archive.GetInputStream($entryPlan.Entry)
                $crc = [ICSharpCode.SharpZipLib.Checksum.Crc32]::new()
                $buffer = [byte[]]::new(65536)
                while (($read = $entryStream.Read($buffer, 0, $buffer.Length)) -gt 0) {
                    if ($clock.Elapsed -gt $ExtractionBudget) { Fail 'Archive extraction exceeded its resource limit.' }
                    $bounded.Write($buffer, 0, $read)
                    $crc.Update([ArraySegment[byte]]::new($buffer, 0, $read))
                }
                $entryStream.Dispose()
                $entryStream = $null
                if ($bounded.BytesWritten -ne $entryPlan.Size) { Fail 'Archive entry output size is invalid.' }
                if ($entryPlan.Entry.AESKeySize -eq 0 -and $crc.Value -ne $entryPlan.Entry.Crc) { Fail 'Archive entry checksum is invalid.' }
                $entryHash = $bounded.HexSha256()
                $bounded.Dispose()
                $bounded = $null
                $destination.Flush($true)
            } finally {
                if ($null -ne $entryStream) { $entryStream.Dispose() }
                if ($null -ne $bounded) { $bounded.Dispose() }
                if ($null -ne $destination) { $destination.Dispose() }
            }
            Set-PrivateFileAcl $target.Path $Context.Sid
            $written.Add([pscustomobject]@{ path = $entryPlan.Path; size = $entryPlan.Size; sha256 = $entryHash })
        }
        $manifest.entries = $written.ToArray()
        $manifest.counts.files = $written.Count
        Write-Manifest $manifestPath $manifest $Context.Sid
        $verified = Test-ArchiveFiles $root $manifest $Context.Sid
        $manifest.state = 'complete'
        $manifest.completedUtc = [DateTime]::UtcNow.ToString('O')
        Write-Manifest $manifestPath $manifest $Context.Sid
        Assert-NoReparse $root
        Assert-PrivateAcl $root $Context.Sid
        Write-Output "Status=Complete Batch=$batchValue Entries=$($verified.Files) Bytes=$($verified.Bytes)"
    } catch {
        if ($rootCreated -and $null -ne $manifest) {
            try {
                $manifest.state = 'failed'
                $manifest.failedUtc = [DateTime]::UtcNow.ToString('O')
                if (Test-Path -LiteralPath $manifestPath -PathType Leaf) { Write-Manifest $manifestPath $manifest $Context.Sid }
            } catch { }
        }
        if ($rootCreated) { Write-Output "Status=Failed Batch=$batchValue" }
        Fail 'Private archive extraction failed.'
    } finally {
        if ($null -ne $archive) { $archive.Close() }
        if ($null -ne $stream) { $stream.Dispose() }
        if ($bstr -ne [IntPtr]::Zero) { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
        $plain = $null
        $Password = $null
    }
}

function Read-ArchiveManifest([pscustomobject] $Context, [string] $RequestedBatch) {
    Assert-Batch $RequestedBatch
    Assert-NoReparse $Context.Base
    Assert-PrivateAcl $Context.Base $Context.Sid -Shallow
    $root = Get-ArchiveRoot $Context $RequestedBatch
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { Fail 'Archive output was not found.' }
    Assert-NoReparse $root
    Assert-PrivateAcl $root $Context.Sid
    $manifestPath = Join-Path $root $ManifestName
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or (Get-Item -LiteralPath $manifestPath -Force).Length -gt 1MB) { Fail 'Archive manifest is invalid.' }
    try { $manifest = [IO.File]::ReadAllText($manifestPath, [Text.UTF8Encoding]::new($false)) | ConvertFrom-Json } catch { Fail 'Archive manifest is invalid.' }
    $required = @('schema', 'batch', 'state', 'createdUtc', 'sourceArchiveSha256', 'directories', 'entries', 'counts', 'completedUtc')
    if ($null -eq $manifest -or @($manifest.PSObject.Properties.Name | Where-Object { $_ -notin $required }).Count -ne 0 -or @($required | Where-Object { $manifest.PSObject.Properties.Name -notcontains $_ }).Count -ne 0 -or [string]$manifest.schema -ne [string]$ManifestSchema -or $manifest.batch -ne $RequestedBatch -or $manifest.state -ne 'complete' -or [string]$manifest.sourceArchiveSha256 -notmatch '^[0-9a-f]{64}$') { Fail 'Archive manifest is invalid.' }
    $completed = [DateTime]::MinValue
    if (-not [DateTime]::TryParse([string]$manifest.createdUtc, [ref]$completed) -or -not [DateTime]::TryParse([string]$manifest.completedUtc, [ref]$completed)) { Fail 'Archive manifest is invalid.' }
    $manifest
}

function Check-PrivateArchive([pscustomobject] $Context) {
    if ([string]::IsNullOrWhiteSpace($Batch)) { Fail 'Batch is required for Check.' }
    if ($script:ArchivePathWasSupplied -or $script:PasswordWasSupplied) { Fail 'Archive input is not accepted for Check.' }
    $manifest = Read-ArchiveManifest $Context $Batch
    $root = Get-ArchiveRoot $Context $Batch
    $verified = Test-ArchiveFiles $root $manifest $Context.Sid
    $counts = $manifest.counts
    $countFields = @('entries', 'directories', 'files', 'bytes')
    $archiveEntries = 0L; $archiveDirectories = 0L; $archiveFiles = 0L; $archiveBytes = 0L
    if ($null -eq $counts -or @($counts.PSObject.Properties.Name | Where-Object { $_ -notin $countFields }).Count -ne 0 -or @($countFields | Where-Object { $counts.PSObject.Properties.Name -notcontains $_ }).Count -ne 0 -or -not [int64]::TryParse([string]$counts.entries, [ref]$archiveEntries) -or -not [int64]::TryParse([string]$counts.directories, [ref]$archiveDirectories) -or -not [int64]::TryParse([string]$counts.files, [ref]$archiveFiles) -or -not [int64]::TryParse([string]$counts.bytes, [ref]$archiveBytes) -or $archiveEntries -ne ($archiveFiles + $archiveDirectories) -or $archiveEntries -gt $MaximumEntries -or $archiveDirectories -ne $verified.Directories -or $archiveFiles -ne $verified.Files -or $archiveBytes -ne $verified.Bytes) { Fail 'Archive manifest is invalid.' }
    Write-Output "Status=Verified Batch=$Batch Entries=$($verified.Files) Bytes=$($verified.Bytes)"
}

try {
    Assert-ArchiveRuntime
    Initialize-ArchiveLibraries
    $context = Get-PrivateImportContext
    if ($Mode -eq 'Extract') { Extract-PrivateArchive $context } else { Check-PrivateArchive $context }
} catch {
    [Console]::Error.WriteLine('Private archive operation failed.')
    exit 1
}
