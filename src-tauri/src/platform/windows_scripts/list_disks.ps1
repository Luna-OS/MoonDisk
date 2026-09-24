# MoonDisk – read-only disk/partition/volume enumeration.
#
# Fixed script, embedded into the binary via include_str! (see
# platform/windows.rs). Takes no parameters and performs no writes; only
# Get-* cmdlets are used. Output is a single JSON document on stdout so
# Rust can parse it the same way it parses `lsblk --json` on Linux.

$ErrorActionPreference = 'SilentlyContinue'

$disks = Get-Disk | ForEach-Object {
    $disk = $_
    $partitions = Get-Partition -DiskNumber $disk.Number | ForEach-Object {
        $part = $_
        $vol = Get-Volume -Partition $part
        [PSCustomObject]@{
            PartitionNumber = $part.PartitionNumber
            Offset          = [int64]$part.Offset
            Size            = [int64]$part.Size
            DriveLetter     = if ($part.DriveLetter) { [string]$part.DriveLetter } else { $null }
            Type            = [string]$part.Type
            GptType         = [string]$part.GptType
            IsBoot          = [bool]$part.IsBoot
            IsSystem        = [bool]$part.IsSystem
            IsActive        = [bool]$part.IsActive
            FileSystem      = if ($vol) { [string]$vol.FileSystem } else { $null }
            FileSystemLabel = if ($vol) { [string]$vol.FileSystemLabel } else { $null }
        }
    }
    [PSCustomObject]@{
        Number         = $disk.Number
        FriendlyName   = [string]$disk.FriendlyName
        Manufacturer   = [string]$disk.Manufacturer
        Model          = [string]$disk.Model
        SerialNumber   = [string]$disk.SerialNumber
        BusType        = [string]$disk.BusType
        Size           = [int64]$disk.Size
        LogicalSectorSize = [int]$disk.LogicalSectorSize
        PartitionStyle = [string]$disk.PartitionStyle
        HealthStatus   = [string]$disk.HealthStatus
        OperationalStatus = [string]$disk.OperationalStatus
        IsReadOnly     = [bool]$disk.IsReadOnly
        IsBoot         = [bool]$disk.IsBoot
        IsSystem       = [bool]$disk.IsSystem
        Partitions     = @($partitions)
    }
}

@($disks) | ConvertTo-Json -Depth 6 -Compress
