param([Parameter(Mandatory)][string]$AppDir,[Parameter(Mandatory)][string]$DataDir,[Parameter(Mandatory)][string]$OwnerAccount)
$ErrorActionPreference = 'Stop'
try {
    $identity = New-Object Security.Principal.NTAccount($OwnerAccount)
    $sid = $identity.Translate([Security.Principal.SecurityIdentifier]).Value
    if ($sid -notmatch '^S-1-5-[0-9-]+$') { throw 'Windows owner account is not valid' }
    New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
    $ownerFile = Join-Path $DataDir 'authorized-owner.sid'
    if (Test-Path -LiteralPath $ownerFile -PathType Leaf) {
        $previous = [IO.File]::ReadAllText($ownerFile).Trim()
        if ($previous -ne $sid) { throw 'This Home Hub is already assigned to a different Windows owner. Keep the existing owner for upgrades.' }
    }
    $system = [Security.Principal.SecurityIdentifier]::new('S-1-5-18')
    $admins = [Security.Principal.SecurityIdentifier]::new('S-1-5-32-544')
    $full = [Security.AccessControl.FileSystemRights]::FullControl
    $allow = [Security.AccessControl.AccessControlType]::Allow
    $inherit = [Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
    $dirAcl = [Security.AccessControl.DirectorySecurity]::new()
    $dirAcl.SetAccessRuleProtection($true,$false)
    foreach ($principal in @($system,$admins)) {
        $dirAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($principal,$full,$inherit,[Security.AccessControl.PropagationFlags]::None,$allow))
    }
    [IO.Directory]::SetAccessControl($DataDir,$dirAcl)
    if (!(Test-Path -LiteralPath $ownerFile -PathType Leaf)) { [IO.File]::WriteAllText($ownerFile,$sid) }
    $fileAcl = [Security.AccessControl.FileSecurity]::new()
    $fileAcl.SetAccessRuleProtection($true,$false)
    foreach ($principal in @($system,$admins)) {
        $fileAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($principal,$full,$allow))
    }
    [IO.File]::SetAccessControl($ownerFile,$fileAcl)
    $bin = Join-Path $AppDir 'hh-service.exe'
    $commandLine = '"' + $bin + '" --data-dir "' + $DataDir + '"'
    if (Get-Service -Name HomeHub -ErrorAction SilentlyContinue) {
        & sc.exe config HomeHub binPath= $commandLine start= auto obj= LocalSystem | Out-Null
    } else {
        & sc.exe create HomeHub binPath= $commandLine start= auto obj= LocalSystem DisplayName= 'Home Hub' | Out-Null
    }
    if ($LASTEXITCODE -ne 0) { throw 'Could not configure the Home Hub service' }
    & sc.exe failure HomeHub reset= 86400 actions= restart/5000/restart/10000/restart/30000 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not configure service recovery' }
    foreach ($rule in @(
        @{ Name='Home Hub API'; Protocol='TCP'; Port='47800'; Program=$bin },
        @{ Name='Home Hub Pairing'; Protocol='TCP'; Port='47802'; Program=$bin },
        @{ Name='Home Hub Discovery'; Protocol='UDP'; Port='5353'; Program=$bin },
        @{ Name='Home Hub Screen'; Protocol='UDP'; Port='Any'; Program=(Join-Path $AppDir 'hh-session.exe') }
    )) {
        Get-NetFirewallRule -DisplayName $rule.Name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
        New-NetFirewallRule -DisplayName $rule.Name -Group 'Home Hub' -Direction Inbound -Action Allow -Profile Private -Protocol $rule.Protocol -LocalPort $rule.Port -RemoteAddress LocalSubnet -Program $rule.Program -EdgeTraversalPolicy Block -ErrorAction Stop | Out-Null
    }
    $launch = Join-Path $AppDir 'start-user.ps1'
    $taskArgs = '-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "' + $launch + '" -AppDir "' + $AppDir + '" -DataDir "' + $DataDir + '"'
    $taskAction = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument $taskArgs
    $taskTrigger = New-ScheduledTaskTrigger -AtLogOn -User $OwnerAccount
    $taskPrincipal = New-ScheduledTaskPrincipal -UserId $OwnerAccount -LogonType Interactive -RunLevel Limited
    Register-ScheduledTask -TaskName 'HomeHub User Session' -Action $taskAction -Trigger $taskTrigger -Principal $taskPrincipal -Force -ErrorAction Stop | Out-Null
    Start-Service -Name HomeHub -ErrorAction Stop
    (Get-Service -Name HomeHub).WaitForStatus('Running',[TimeSpan]::FromSeconds(30))
    Start-ScheduledTask -TaskName 'HomeHub User Session' -ErrorAction SilentlyContinue
} catch {
    Write-Error $_
    exit 1
}
