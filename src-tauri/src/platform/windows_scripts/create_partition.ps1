# MoonDisk – create a partition. All values arrive as typed script
# parameters (bound positionally/by name by PowerShell itself, the same
# way Rust's std::process::Command passes each argv entry as one atomic
# string) — never concatenated into script text, so there is nothing here
# for a label or size to "escape out of".
param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int64]  $OffsetBytes,
    [Parameter(Mandatory)] [int64]  $SizeBytes,
    [string]                        $DriveLetter = ''
)

$ErrorActionPreference = 'Stop'

if ($DriveLetter -ne '') {
    New-Partition -DiskNumber $DiskNumber -Offset $OffsetBytes -Size $SizeBytes `
        -DriveLetter $DriveLetter[0] | Out-Null
} else {
    New-Partition -DiskNumber $DiskNumber -Offset $OffsetBytes -Size $SizeBytes `
        -AssignDriveLetter:$false | Out-Null
}

Write-Output 'OK'
