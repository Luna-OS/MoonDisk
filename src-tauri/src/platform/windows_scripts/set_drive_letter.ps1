param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int]    $PartitionNumber,
    [Parameter(Mandatory)] [string] $DriveLetter
)

$ErrorActionPreference = 'Stop'

$letter = [string]$DriveLetter[0]
$partition = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber

# Set-Partition -NewDriveLetter only remaps an existing letter; a partition
# that has none yet needs an access path added instead.
if ([string]$partition.DriveLetter -match '^[A-Za-z]$') {
    Set-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber -NewDriveLetter $letter
} else {
    Add-PartitionAccessPath -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber -AccessPath "${letter}:\"
}

$after = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber
if ([string]$after.DriveLetter -ne $letter) {
    throw "Laufwerksbuchstabe ${letter}: wurde nicht zugewiesen (ist er schon belegt?)"
}

Write-Output 'OK'
