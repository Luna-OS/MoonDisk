param(
    [Parameter(Mandatory)] [int] $DiskNumber,
    [Parameter(Mandatory)] [int] $PartitionNumber
)

$ErrorActionPreference = 'Stop'

Remove-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber -Confirm:$false

Write-Output 'OK'
