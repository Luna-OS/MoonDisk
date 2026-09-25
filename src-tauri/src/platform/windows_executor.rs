//! Real Windows write executor — **untested**, see the module-level note
//! in `platform::windows`. Every write is one embedded, fixed `.ps1`
//! script (`platform/windows_scripts/`) invoked through
//! `super::run_powershell_script`, with dynamic values passed as separate
//! typed script parameters, never concatenated into script text.

use super::run_powershell_script;
use super::windows::WindowsDiskProvider;
use super::DiskInventory;
use crate::models::FileSystem;
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};

const CREATE_PARTITION_SCRIPT: &str = include_str!("windows_scripts/create_partition.ps1");
const DELETE_PARTITION_SCRIPT: &str = include_str!("windows_scripts/delete_partition.ps1");
const FORMAT_PARTITION_SCRIPT: &str = include_str!("windows_scripts/format_partition.ps1");
const SET_LABEL_SCRIPT: &str = include_str!("windows_scripts/set_label.ps1");
const SET_DRIVE_LETTER_SCRIPT: &str = include_str!("windows_scripts/set_drive_letter.ps1");

pub struct WindowsDiskExecutor;

impl DiskOperationExecutor for WindowsDiskExecutor {
    fn execute(
        &mut self,
        req: &OperationRequest,
        confirmation: Confirmation,
    ) -> Result<(), ExecutionError> {
        let provider = WindowsDiskProvider;
        let disk = provider.disk(&req.disk_id())?;
        validate(&disk, req)?;
        if req.risk() >= RiskLevel::High && !confirmation.confirmed {
            return Err(ExecutionError::ConfirmationMissing);
        }

        let disk_number = disk_number_from_id(&disk.id.0)?;

        match req {
            OperationRequest::CreatePartition {
                start,
                size,
                filesystem,
                label,
                drive_letter,
                ..
            } => {
                let fs_name = windows_fs_name(*filesystem)?;
                let mut args = vec![
                    "-DiskNumber".to_string(),
                    disk_number.to_string(),
                    "-OffsetBytes".to_string(),
                    start.0.to_string(),
                    "-SizeBytes".to_string(),
                    size.0.to_string(),
                    "-FileSystem".to_string(),
                    fs_name.to_string(),
                ];
                if let Some(l) = label {
                    args.push("-Label".to_string());
                    args.push(l.clone());
                }
                if let Some(l) = drive_letter {
                    args.push("-DriveLetter".to_string());
                    args.push(l.to_string());
                }
                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                run_powershell_script(CREATE_PARTITION_SCRIPT, &arg_refs)
                    .map_err(|e| ExecutionError::Failed(e.to_string()))?;
            }
            OperationRequest::DeletePartition { partition } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                run_powershell_script(
                    DELETE_PARTITION_SCRIPT,
                    &[
                        "-DiskNumber",
                        &disk_number.to_string(),
                        "-PartitionNumber",
                        &p.number.to_string(),
                    ],
                )
                .map_err(|e| ExecutionError::Failed(e.to_string()))?;
            }
            OperationRequest::FormatPartition {
                partition,
                filesystem,
                label,
            } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                let fs_name = windows_fs_name(*filesystem)?;
                let mut args = vec![
                    "-DiskNumber".to_string(),
                    disk_number.to_string(),
                    "-PartitionNumber".to_string(),
                    p.number.to_string(),
                    "-FileSystem".to_string(),
                    fs_name.to_string(),
                ];
                if let Some(l) = label {
                    args.push("-Label".to_string());
                    args.push(l.clone());
                }
                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                run_powershell_script(FORMAT_PARTITION_SCRIPT, &arg_refs)
                    .map_err(|e| ExecutionError::Failed(e.to_string()))?;
            }
            OperationRequest::SetLabel { partition, label } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                run_powershell_script(
                    SET_LABEL_SCRIPT,
                    &[
                        "-DiskNumber",
                        &disk_number.to_string(),
                        "-PartitionNumber",
                        &p.number.to_string(),
                        "-Label",
                        label,
                    ],
                )
                .map_err(|e| ExecutionError::Failed(e.to_string()))?;
            }
            OperationRequest::SetDriveLetter {
                partition,
                drive_letter,
            } => {
                let p = disk
                    .partitions()
                    .find(|p| &p.id == partition)
                    .expect("validated above: partition exists on this disk");
                run_powershell_script(
                    SET_DRIVE_LETTER_SCRIPT,
                    &[
                        "-DiskNumber",
                        &disk_number.to_string(),
                        "-PartitionNumber",
                        &p.number.to_string(),
                        "-DriveLetter",
                        &drive_letter.to_string(),
                    ],
                )
                .map_err(|e| ExecutionError::Failed(e.to_string()))?;
            }
        }
        Ok(())
    }
}

fn disk_number_from_id(id: &str) -> Result<u32, ExecutionError> {
    id.rsplit("PhysicalDrive")
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ExecutionError::Failed(format!("ungültige Datenträger-ID: {id}")))
}

fn windows_fs_name(fs: FileSystem) -> Result<&'static str, ExecutionError> {
    match fs {
        FileSystem::Ntfs => Ok("NTFS"),
        FileSystem::Fat32 => Ok("FAT32"),
        FileSystem::ExFat => Ok("exFAT"),
        _ => Err(ExecutionError::NotImplemented(
            "Windows kann dieses Dateisystem nicht ohne Zusatzsoftware formatieren".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disk_number_from_physical_drive_id() {
        assert_eq!(disk_number_from_id(r"\\.\PhysicalDrive0").unwrap(), 0);
        assert_eq!(disk_number_from_id(r"\\.\PhysicalDrive12").unwrap(), 12);
        assert!(disk_number_from_id("nonsense").is_err());
    }
}
