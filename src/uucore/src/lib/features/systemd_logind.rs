// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
//
// spell-checker:ignore logind

//! Systemd-logind support for reading login records
//!
//! This module provides systemd-logind based implementation for reading
//! login records as an alternative to traditional utmp/utmpx files.
//! When the systemd-logind feature is enabled and systemd is available,
//! this will be used instead of traditional utmp files.

use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{UResult, USimpleError};
use crate::utmpx;

use systemd::login;

/// Login record compatible with utmpx structure
#[derive(Debug, Clone)]
pub struct SystemdLoginRecord {
    pub user: String,
    pub session_id: String,
    pub seat_or_tty: String,
    pub host: String,
    pub login_time: SystemTime,
    pub pid: u32,
    pub session_leader_pid: u32,
    pub record_type: SystemdRecordType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemdRecordType {
    UserProcess = 7,  // USER_PROCESS
    LoginProcess = 6, // LOGIN_PROCESS
    BootTime = 2,     // BOOT_TIME
}

impl SystemdLoginRecord {
    /// Check if this is a user process record
    pub fn is_user_process(&self) -> bool {
        !self.user.is_empty() && self.record_type == SystemdRecordType::UserProcess
    }

    /// Get login time as time::OffsetDateTime compatible with utmpx
    pub fn login_time_offset(&self) -> utmpx::time::OffsetDateTime {
        let duration = self
            .login_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let ts_nanos: i128 = (duration.as_nanos()).try_into().unwrap_or(0);
        let local_offset = utmpx::time::OffsetDateTime::now_local()
            .map_or_else(|_| utmpx::time::UtcOffset::UTC, |v| v.offset());
        utmpx::time::OffsetDateTime::from_unix_timestamp_nanos(ts_nanos)
            .unwrap_or_else(|_| {
                utmpx::time::OffsetDateTime::now_local()
                    .unwrap_or_else(|_| utmpx::time::OffsetDateTime::now_utc())
            })
            .to_offset(local_offset)
    }
}

/// Read login records from systemd-logind using safe wrapper functions
/// This matches the approach used by GNU coreutils read_utmp_from_systemd()
pub fn read_login_records() -> UResult<Vec<SystemdLoginRecord>> {
    let mut records = Vec::new();

    // Get all active sessions using safe wrapper
    let sessions = match login::get_sessions() {
        Ok(sessions) => sessions,
        Err(e) => {
            return Err(USimpleError::new(
                1,
                format!("Failed to get systemd sessions: {}", e),
            ));
        }
    };

    // Iterate through all sessions
    for session_id in sessions {
        // Get session UID using safe wrapper
        let uid = match login::get_session_uid(&session_id) {
            Ok(uid) => uid,
            Err(_) => continue, // Skip sessions we can't get UID for
        };

        // Get username from UID
        let user = unsafe {
            let passwd = libc::getpwuid(uid);
            if passwd.is_null() {
                format!("{}", uid) // fallback to UID if username not found
            } else {
                CStr::from_ptr((*passwd).pw_name)
                    .to_string_lossy()
                    .into_owned()
            }
        };

        // Get start time using safe wrapper
        let start_time = login::get_session_start_time(&session_id)
            .map(|usec| UNIX_EPOCH + std::time::Duration::from_micros(usec))
            .unwrap_or(UNIX_EPOCH); // fallback to epoch if unavailable

        // Get TTY using safe wrapper
        let tty = login::get_session_tty(&session_id)
            .ok()
            .flatten()
            .unwrap_or_default();

        // Get remote host using safe wrapper
        let remote_host = login::get_session_remote_host(&session_id)
            .ok()
            .flatten()
            .unwrap_or_default();

        // Get display using safe wrapper (for GUI sessions)
        let display = login::get_session_display(&session_id)
            .ok()
            .flatten()
            .unwrap_or_default();

        // Get session type using safe wrapper (currently unused but available)
        let _session_type = login::get_session_type(&session_id)
            .ok()
            .flatten()
            .unwrap_or_default();

        // Determine the seat/tty value (prefer tty, fallback to display)
        let seat_or_tty = if !tty.is_empty() {
            tty
        } else if !display.is_empty() {
            display
        } else {
            "?".to_string() // fallback
        };

        // Determine host (use remote_host if available)
        let host = if !remote_host.is_empty() {
            remote_host
        } else {
            String::new()
        };

        // Create the record
        let record = SystemdLoginRecord {
            user,
            session_id: session_id.clone(),
            seat_or_tty,
            host,
            login_time: start_time,
            pid: 0, // systemd doesn't directly provide session leader PID in this context
            session_leader_pid: 0,
            record_type: SystemdRecordType::UserProcess, // Most sessions are user processes
        };

        records.push(record);
    }

    Ok(records)
}

/// Wrapper to provide utmpx-compatible interface for a single record
pub struct SystemdUtmpxCompat {
    record: SystemdLoginRecord,
}

impl SystemdUtmpxCompat {
    /// Create new instance from a SystemdLoginRecord
    pub fn new(record: SystemdLoginRecord) -> Self {
        SystemdUtmpxCompat { record }
    }

    /// A.K.A. ut.ut_type
    pub fn record_type(&self) -> i16 {
        self.record.record_type as i16
    }

    /// A.K.A. ut.ut_pid
    pub fn pid(&self) -> i32 {
        self.record.pid as i32
    }

    /// A.K.A. ut.ut_id
    pub fn terminal_suffix(&self) -> String {
        // Extract last part of session ID or use session ID
        self.record.session_id.clone()
    }

    /// A.K.A. ut.ut_user
    pub fn user(&self) -> String {
        self.record.user.clone()
    }

    /// A.K.A. ut.ut_host
    pub fn host(&self) -> String {
        self.record.host.clone()
    }

    /// A.K.A. ut.ut_line
    pub fn tty_device(&self) -> String {
        self.record.seat_or_tty.clone()
    }

    /// Login time
    pub fn login_time(&self) -> utmpx::time::OffsetDateTime {
        self.record.login_time_offset()
    }

    /// Exit status (not available from systemd)
    pub fn exit_status(&self) -> (i16, i16) {
        (0, 0) // Not available from systemd
    }

    /// Check if this is a user process record
    pub fn is_user_process(&self) -> bool {
        self.record.is_user_process()
    }

    /// Canonical host name
    pub fn canon_host(&self) -> std::io::Result<String> {
        // Simple implementation - just return the host as-is
        // Could be enhanced with DNS lookup like the original
        Ok(self.record.host.clone())
    }
}

/// Container for reading multiple systemd records
pub struct SystemdUtmpxIter {
    records: Vec<SystemdLoginRecord>,
    current_index: usize,
}

impl SystemdUtmpxIter {
    /// Create new instance and read records from systemd-logind
    pub fn new() -> UResult<Self> {
        let records = read_login_records()?;
        Ok(SystemdUtmpxIter {
            records,
            current_index: 0,
        })
    }

    /// Get next record (similar to getutxent)
    pub fn next_record(&mut self) -> Option<SystemdUtmpxCompat> {
        if self.current_index >= self.records.len() {
            return None;
        }

        let record = self.records[self.current_index].clone();
        self.current_index += 1;

        // Return SystemdUtmpxCompat
        Some(SystemdUtmpxCompat::new(record))
    }

    /// Get all records at once
    pub fn get_all_records(&self) -> Vec<SystemdUtmpxCompat> {
        self.records
            .iter()
            .cloned()
            .map(SystemdUtmpxCompat::new)
            .collect()
    }

    /// Reset iterator to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get number of records
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl Iterator for SystemdUtmpxIter {
    type Item = SystemdUtmpxCompat;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_record()
    }
}
