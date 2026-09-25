param(
    [Parameter(Mandatory)] [int] $DiskNumber
)

$ErrorActionPreference = 'Stop'

# Windows refuses raw writes to sectors that belong to a mounted volume.
# Removing all partitions first leaves no volumes on the disk, so the
# image can be written to \\.\PhysicalDriveN from start to end.
$disk = Get-Disk -Number $DiskNumber
if ($disk.IsOffline) {
    Set-Disk -Number $DiskNumber -IsOffline $false
}
if ($disk.PartitionStyle -ne 'RAW') {
    Clear-Disk -Number $DiskNumber -RemoveData -RemoveOEM -Confirm:$false
}

Write-Output 'OK'
