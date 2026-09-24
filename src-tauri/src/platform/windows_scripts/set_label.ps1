param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int]    $PartitionNumber,
    [Parameter(Mandatory)] [string] $Label
)

$ErrorActionPreference = 'Stop'

$partition = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber
Get-Volume -Partition $partition | Set-Volume -NewFileSystemLabel $Label

Write-Output 'OK'
