param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int]    $PartitionNumber,
    [Parameter(Mandatory)] [string] $DriveLetter
)

$ErrorActionPreference = 'Stop'

# -NewDriveLetter both assigns a letter to a partition that has none yet
# and remaps an existing one, so this one cmdlet covers both cases MoonDisk
# needs: freshly created partitions (which MoonDisk creates with no letter)
# and changing an already-assigned letter.
Set-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber -NewDriveLetter $DriveLetter[0]

Write-Output 'OK'
