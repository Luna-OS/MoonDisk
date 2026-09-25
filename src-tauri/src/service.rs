//! What every front end does with disks — the Tauri app as well as the
//! helper process the native macOS app talks to (see `helper`): list
//! disks, execute one operation, and write an image with throttled
//! progress.
//!
//! Executing always re-fetches the real current state of the disk,
//! re-validates the request against it and requires an explicit
//! confirmation for anything at High risk or above — whatever the front
//! end already showed the user.

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
compile_error!("MoonDisk only supports Windows, Linux and macOS");

use crate::flash::{self, Phase, WriteMode};
use crate::models::{ByteSize, Disk, DiskId};
use crate::operations::{
    validate, Confirmation, DiskOperationExecutor, ExecutionError, OperationRequest, RiskLevel,
};
use crate::platform::DiskInventory;
use serde::Serialize;
use std::fs::File;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub fn inventory() -> Box<dyn DiskInventory> {
    #[cfg(target_os = "linux")]
    {
        Box::new(crate::platform::linux::LinuxDiskProvider)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(crate::platform::windows::WindowsDiskProvider)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(crate::platform::macos::MacDiskProvider)
    }
}

pub fn executor() -> Box<dyn DiskOperationExecutor> {
    #[cfg(target_os = "linux")]
    {
        Box::new(crate::platform::linux_executor::LinuxDiskExecutor)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(crate::platform::windows_executor::WindowsDiskExecutor)
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(crate::platform::macos_executor::MacDiskExecutor)
    }
}

/// "windows", "linux" or "macos".
pub fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

pub fn list_disks() -> Result<Vec<Disk>, String> {
    inventory().list_disks().map_err(|e| e.to_string())
}

fn disk(id: &DiskId) -> Result<Disk, String> {
    inventory()
        .disk(id)
        .map_err(|e| ExecutionError::from(e).to_string())
}

/// Validates and, if it passes, executes a single operation. `confirmed`
/// must be `true` for anything the request itself classifies as
/// `RiskLevel::High` or above.
pub fn execute(request: &OperationRequest, confirmed: bool) -> Result<(), String> {
    let disk = disk(&request.disk_id())?;
    validate(&disk, request)
        .map_err(ExecutionError::from)
        .map_err(|e| e.to_string())?;
    if request.risk() >= RiskLevel::High && !confirmed {
        return Err(ExecutionError::ConfirmationMissing.to_string());
    }
    executor()
        .execute(request, Confirmation { confirmed })
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashProgress {
    pub phase: Phase,
    pub done: ByteSize,
    pub total: ByteSize,
    pub bytes_per_second: f64,
}

/// Writes `image` onto the disk `disk_id`, calling `emit` at most every
/// 100 ms (plus once at the end of every phase).
pub fn flash(
    image: &mut File,
    disk_id: &str,
    mode: WriteMode,
    verify: bool,
    cancel: &AtomicBool,
    mut emit: impl FnMut(FlashProgress),
) -> Result<(), String> {
    let disk = disk(&DiskId::from(disk_id.to_string()))?;
    let mut phase = Phase::Preparing;
    let mut phase_started = Instant::now();
    let mut last_emit: Option<Instant> = None;
    flash::run(image, &disk, mode, verify, cancel, |p, done, total| {
        let now = Instant::now();
        if p != phase {
            phase = p;
            phase_started = now;
            last_emit = None;
        }
        let due = last_emit.map_or(true, |t| now - t >= Duration::from_millis(100));
        if !due && done < total {
            return;
        }
        last_emit = Some(now);
        let secs = (now - phase_started).as_secs_f64();
        emit(FlashProgress {
            phase: p,
            done: ByteSize(done),
            total: ByteSize(total),
            bytes_per_second: if secs > 0.0 { done as f64 / secs } else { 0.0 },
        });
    })
    .map_err(|e| e.to_string())
}
