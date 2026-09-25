# MoonDisk – create a partition. All values arrive as typed script
# parameters (bound positionally/by name by PowerShell itself, the same
# way Rust's std::process::Command passes each argv entry as one atomic
# string) — never concatenated into script text, so there is nothing here
# for a label or size to "escape out of".
param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int64]  $OffsetBytes,
    [Parameter(Mandatory)] [int64]  $SizeBytes,
    [Parameter(Mandatory)] [string] $FileSystem,
    [string]                        $Label = '',
    [string]                        $DriveLetter = ''
)

$ErrorActionPreference = 'Stop'

$disk = Get-Disk -Number $DiskNumber
$justInitialized = $false
if ($disk.PartitionStyle -eq 'RAW') {
    # A disk with no partition table at all can't take a New-Partition
    # call yet - Windows needs GPT/MBR headers written first (this is
    # what "Initialize Disk" does in Disk Management). MoonDisk is
    # GPT-first everywhere else, so GPT is the only style it initializes.
    Initialize-Disk -Number $DiskNumber -PartitionStyle GPT -Confirm:$false
    $justInitialized = $true
}

$params = @{ DiskNumber = $DiskNumber }

if ($justInitialized) {
    # The offset/size MoonDisk sent were computed from the *raw* disk
    # before this script ran, since a disk with no partition table has
    # no real free-space map to compute them from in the first place -
    # they don't account for the few MiB Initialize-Disk itself just
    # reserved for the GPT header. Re-query the disk's actual free space
    # now that it has a real partition table and place the partition at
    # the start of that (letting New-Partition pick the offset), capping
    # the requested size to what genuinely fits.
    $disk = Get-Disk -Number $DiskNumber
    $params['Size'] = [Math]::Min($SizeBytes, $disk.LargestFreeExtent)
} else {
    $params['Offset'] = $OffsetBytes
    $params['Size'] = $SizeBytes
}

$params['AssignDriveLetter'] = $false
$new = New-Partition @params
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

# The letter is added only after formatting, and checked afterwards:
# `New-Partition -DriveLetter` could leave a partition without a letter
# and without an error, so it never showed up in Explorer.
if ($DriveLetter -ne '') {
    $letter = [string]$DriveLetter[0]
    Add-PartitionAccessPath -DiskNumber $DiskNumber -PartitionNumber $number -AccessPath "${letter}:\"
    $after = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $number
    if ([string]$after.DriveLetter -ne $letter) {
        throw "Partition erstellt, aber Laufwerksbuchstabe ${letter}: wurde nicht zugewiesen (ist er schon belegt?)"
    }
}

Write-Output 'OK'
