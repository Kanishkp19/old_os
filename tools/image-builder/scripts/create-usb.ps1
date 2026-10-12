param(
    [string]$Image,
    [int]$DiskNumber = -1,
    [switch]$List,
    [switch]$Force,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'

try {
    if ($List) {
        Get-Disk | Where-Object { $_.BusType -eq 'USB' } |
            Select-Object Number, FriendlyName, Model, Size, SerialNumber, BusType |
            Format-Table -AutoSize
        return
    }
    if ([string]::IsNullOrWhiteSpace($Image)) { throw 'Missing -Image' }
    if ($DiskNumber -lt 0) { throw 'Missing -DiskNumber' }
    $source = Get-Item -LiteralPath $Image -ErrorAction Stop
    if ($source.PSIsContainer -or $source.Length -le 0) { throw 'Image is empty or is not a file' }
    $disk = Get-Disk -Number $DiskNumber -ErrorAction Stop
    if ($disk.IsBoot -or $disk.IsSystem) { throw 'Refusing to write to system root disk' }
    if ($disk.BusType -ne 'USB') { throw 'Target must be a USB disk' }
    if ($disk.Size -lt $source.Length) { throw 'Image is larger than target disk' }
    if ($disk.IsReadOnly -or $disk.IsOffline) { throw 'Target disk must be online and writable' }

    Write-Host "Target: PhysicalDrive$DiskNumber"
    Write-Host "Model: $($disk.Model)"
    Write-Host "Vendor: $($disk.Manufacturer)"
    Write-Host "FriendlyName: $($disk.FriendlyName)"
    Write-Host "SerialNumber: $($disk.SerialNumber)"
    Write-Host "Size: $($disk.Size) bytes"
    Write-Host "Image: $($source.FullName) ($($source.Length) bytes)"
    $partitions = @(Get-Partition -DiskNumber $DiskNumber -ErrorAction Stop)
    foreach ($partition in $partitions) {
        Write-Host "Existing partition $($partition.PartitionNumber): drive letter $($partition.DriveLetter)"
        if (@($partition.AccessPaths | Where-Object { $_ -and $_ -notmatch '^[A-Za-z]:\\$' -and $_ -notmatch '^\\\\\?\\Volume\{' }).Count -gt 0) {
            throw 'Target has a mounted volume without a drive letter; remove its mount point first'
        }
    }
    if ($DryRun) {
        Write-Host 'Dry run: validation passed; no bytes written.'
        return
    }
    if (-not $Force) {
        $answer = Read-Host "Type 'YES' to confirm erasing PhysicalDrive$DiskNumber ($($disk.Size) bytes) and writing $($source.FullName)"
        if ($answer -cne 'YES') { throw 'Confirmation declined' }
    }

    # Mounted drive letters keep Windows filesystem handles open. Remove their
    # mount points after confirmation so the raw disk can be opened exclusively.
    foreach ($partition in $partitions) {
        if ($partition.DriveLetter) {
            $mountPoint = "$($partition.DriveLetter):\"
            & mountvol $mountPoint /p | Out-Null
            if ($LASTEXITCODE -ne 0) { throw "Could not dismount $mountPoint" }
        }
    }

    $sourceHash = (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash
    $drivePath = "\\.\PhysicalDrive$DiskNumber"
    $block = New-Object byte[] (4MB)
    $inputStream = $null
    $diskStream = $null
    try {
        $inputStream = [System.IO.File]::OpenRead($source.FullName)
        $diskStream = [System.IO.FileStream]::new($drivePath, [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None, $block.Length,
            [System.IO.FileOptions]::WriteThrough)
        while (($count = $inputStream.Read($block, 0, $block.Length)) -gt 0) {
            $diskStream.Write($block, 0, $count)
        }
        $diskStream.Flush($true)
    }
    finally {
        if ($diskStream) { $diskStream.Dispose() }
        if ($inputStream) { $inputStream.Dispose() }
    }

    $diskStream = $null
    $sha = $null
    try {
        $diskStream = [System.IO.FileStream]::new($drivePath, [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite, $block.Length)
        $sha = [System.Security.Cryptography.SHA256]::Create()
        [long]$remaining = $source.Length
        while ($remaining -gt 0) {
            $wanted = [int][Math]::Min($remaining, $block.Length)
            $count = $diskStream.Read($block, 0, $wanted)
            if ($count -le 0) { throw 'Verification failed: short read from target disk' }
            [void]$sha.TransformBlock($block, 0, $count, $block, 0)
            $remaining -= $count
        }
        [void]$sha.TransformFinalBlock($block, 0, 0)
        $readbackHash = [BitConverter]::ToString($sha.Hash).Replace('-', '')
    }
    finally {
        if ($sha) { $sha.Dispose() }
        if ($diskStream) { $diskStream.Dispose() }
    }
    if ($sourceHash -ne $readbackHash) { throw 'Verification failed: hash mismatch' }
    Write-Host 'Verification passed. Bootable USB created successfully.'
}
catch {
    [Console]::Error.WriteLine("FATAL: $($_.Exception.Message)")
    exit 1
}
