# MoonDisk – erase a whole disk and leave one formatted partition spanning
# it (e.g. to turn a USB stick an image was written to back into a normal
# drive). Values arrive as typed script parameters, never as script text.
param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [string] $FileSystem,
    [string]                        $Label = ''
)

$ErrorActionPreference = 'Stop'

$disk = Get-Disk -Number $DiskNumber
if ($disk.IsOffline) {
    Set-Disk -Number $DiskNumber -IsOffline $false
}
if ($disk.PartitionStyle -ne 'RAW') {
    Clear-Disk -Number $DiskNumber -RemoveData -RemoveOEM -Confirm:$false
}

# MBR is what every OS and firmware reads on a USB stick; it only tops out
# at 2 TiB.
$disk = Get-Disk -Number $DiskNumber
if ($disk.PartitionStyle -eq 'RAW') {
    $style = if ($disk.Size -gt 2TB) { 'GPT' } else { 'MBR' }
    Initialize-Disk -Number $DiskNumber -PartitionStyle $style
}

$new = New-Partition -DiskNumber $DiskNumber -UseMaximumSize -AssignDriveLetter:$false
$number = $new.PartitionNumber

$formatParams = @{
    FileSystem = $FileSystem
    Confirm    = $false
    Force      = $true
}
if ($Label -ne '') {
    $formatParams['NewFileSystemLabel'] = $Label
}
Get-Partition -DiskNumber $DiskNumber -PartitionNumber $number | Format-Volume @formatParams | Out-Null

# Give it a drive letter so it shows up in Explorer.
$after = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $number
if (-not $after.DriveLetter) {
    Add-PartitionAccessPath -DiskNumber $DiskNumber -PartitionNumber $number -AssignDriveLetter
}

Write-Output 'OK'
