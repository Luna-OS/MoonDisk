param(
    [Parameter(Mandatory)] [int]    $DiskNumber,
    [Parameter(Mandatory)] [int]    $PartitionNumber,
    [Parameter(Mandatory)] [string] $FileSystem,
    [string]                        $Label = ''
)

$ErrorActionPreference = 'Stop'

$partition = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $PartitionNumber

$params = @{
    FileSystem  = $FileSystem
    Confirm     = $false
    Force       = $true
}
if ($Label -ne '') {
    $params['NewFileSystemLabel'] = $Label
}

$partition | Format-Volume @params | Out-Null

Write-Output 'OK'
