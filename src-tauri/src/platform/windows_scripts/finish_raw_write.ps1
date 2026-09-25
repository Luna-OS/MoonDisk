param(
    [Parameter(Mandatory)] [int] $DiskNumber
)

$ErrorActionPreference = 'Stop'

# Make Windows re-read the partition table the image just brought along.
Update-Disk -Number $DiskNumber

Write-Output 'OK'
