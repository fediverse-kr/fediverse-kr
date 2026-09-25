# Side-effect-free private import path and ACL primitives.

function Fail([string] $Message) { throw $Message }

function Get-PrivateImportContext {
    $localAppData = [Environment]::GetFolderPath([Environment+SpecialFolder]::LocalApplicationData)
    if ([string]::IsNullOrWhiteSpace($localAppData)) { Fail 'Known LocalAppData is unavailable.' }
    $base = [IO.Path]::GetFullPath((Join-Path $localAppData 'fediversekr\private-import')).TrimEnd('\')
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    if ([string]::IsNullOrWhiteSpace($sid)) { Fail 'Current user SID is unavailable.' }
    [pscustomobject]@{ Base = $base; Sid = $sid }
}

function Assert-Batch([string] $Value) {
    if ($Value -notmatch '^[0-9a-f]{32}$') { Fail 'Batch must be a 32-character lowercase hexadecimal UUID.' }
}

function Assert-NoReparse([string] $Path) {
    $cursor = [IO.DirectoryInfo]::new($Path)
    while ($null -ne $cursor) {
        if ($cursor.Exists -and (($cursor.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
            Fail 'A path ancestor is a reparse point.'
        }
        $parent = $cursor.Parent
        if ($null -eq $parent -or $cursor.FullName -eq [IO.Path]::GetPathRoot($cursor.FullName)) { break }
        $cursor = $parent
    }
}

function Set-PrivateDirectoryAcl([string] $Path, [string] $Sid) {
    [IO.Directory]::CreateDirectory($Path) | Out-Null
    $security = [Security.AccessControl.DirectorySecurity]::new()
    $security.SetAccessRuleProtection($true, $false)
    $inherit = [Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [Security.AccessControl.InheritanceFlags]::ObjectInherit
    $full = [Security.AccessControl.FileSystemRights]::FullControl
    $userIdentity = [Security.Principal.SecurityIdentifier]::new($Sid)
    $systemIdentity = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($userIdentity, $full, $inherit, [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow))
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($systemIdentity, $full, $inherit, [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow))
    # Persist only the DACL. Set-Acl may try to write owner/SACL sections on
    # an existing LocalAppData directory and require SeSecurityPrivilege.
    [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($Path), $security)
}

function Set-PrivateFileAcl([string] $Path, [string] $Sid) {
    $security = [Security.AccessControl.FileSecurity]::new()
    $security.SetAccessRuleProtection($true, $false)
    $full = [Security.AccessControl.FileSystemRights]::FullControl
    $userIdentity = [Security.Principal.SecurityIdentifier]::new($Sid)
    $systemIdentity = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($userIdentity, $full, [Security.AccessControl.AccessControlType]::Allow))
    $security.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($systemIdentity, $full, [Security.AccessControl.AccessControlType]::Allow))
    [IO.FileSystemAclExtensions]::SetAccessControl([IO.FileInfo]::new($Path), $security)
}

function Assert-PrivateAcl([string] $Root, [string] $Sid, [switch] $Shallow) {
    $allowed = @($Sid, 'S-1-5-18')
    $rootItem = Get-Item -LiteralPath $Root -Force -ErrorAction Stop
    if (-not ($rootItem -is [IO.DirectoryInfo])) { Fail 'Cluster root is not a directory.' }
    $items = @($rootItem)
    if (-not $Shallow) { $items += @(Get-ChildItem -LiteralPath $Root -Force -Recurse -ErrorAction Stop) }
    foreach ($item in $items) {
        # PostgreSQL can remove a transient status file while the recursive
        # check is enumerating. Re-check that exact item, but never weaken ACL
        # acceptance for an item that still exists.
        try {
            $item = Get-Item -LiteralPath $item.FullName -Force -ErrorAction Stop
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { Fail 'Cluster contains a reparse point.' }
            $acl = Get-Acl -LiteralPath $item.FullName -ErrorAction Stop
        } catch [System.Management.Automation.ItemNotFoundException] { continue }
        catch [System.IO.FileNotFoundException] { continue }
        if ($item.FullName -eq $Root -and -not $acl.AreAccessRulesProtected) { Fail 'Cluster root ACL is inheritable.' }
        $seen = @{}
        foreach ($rule in $acl.Access) {
            try {
                $identity = $rule.IdentityReference
                $ruleSid = if ($identity -is [Security.Principal.SecurityIdentifier]) { $identity.Value } else { $identity.Translate([Security.Principal.SecurityIdentifier]).Value }
            } catch { Fail 'Cluster ACL identity could not be resolved.' }
            if ($allowed -notcontains $ruleSid -or $rule.AccessControlType -ne 'Allow' -or (($rule.FileSystemRights -band [Security.AccessControl.FileSystemRights]::FullControl) -ne [Security.AccessControl.FileSystemRights]::FullControl)) {
                Fail 'Cluster ACL contains an unexpected entry.'
            }
            $seen[$ruleSid] = $true
        }
        foreach ($expected in $allowed) { if (-not $seen.ContainsKey($expected)) { Fail 'Cluster ACL is missing an expected owner entry.' } }
    }
}
