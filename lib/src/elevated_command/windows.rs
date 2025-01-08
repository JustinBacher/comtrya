/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Luis Liu. All rights reserved.
 *  Licensed under the MIT License. See License in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

use super::Command;
use anyhow::Result;
use std::ffi::OsString;
use std::io::{BufRead, BufReader};
use std::mem;
use std::os::windows::ffi::{OsStrExt as _, OsStringExt};
use std::os::windows::io::FromRawHandle;
use std::os::windows::process::ExitStatusExt;
use std::process::{Child, Command as StdCommand, ExitStatus, Output, Stdio};
use std::{mem, thread};
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
use winapi::um::securitybaseapi::GetTokenInformation;
use winapi::um::winnt::{TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{GetLastError, HANDLE as FHANDLE, HWND};
use windows::Win32::Security::SECURITY_ATTRIBUTES;
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{
    CreateProcessW, WaitForSingleObject, INFINITE, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
    STARTF_USESTDHANDLES, STARTUPINFOW,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::Shell::{
    ShellExecuteExW, ShellExecuteW, SEE_MASK_NOCLOSEPROCESS, SEE_MASK_UNICODE, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

/// The implementation of state check and elevated executing varies on each platform
impl Command {
    /// Check the state the current program running
    ///
    /// Return `true` if the program is running as root, otherwise false
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use elevated_command::Command;
    ///
    /// fn main() -> io::Result<()> {
    ///     let is_elevated = Command::is_elevated();
    ///
    /// }
    /// ```
    pub fn is_elevated() -> bool {
        // Thanks to https://stackoverflow.com/a/8196291
        unsafe {
            let mut current_token_ptr: HANDLE = mem::zeroed();
            let mut token_elevation: TOKEN_ELEVATION = mem::zeroed();
            let token_elevation_type_ptr: *mut TOKEN_ELEVATION = &mut token_elevation;
            let mut size: DWORD = 0;

            let result = OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut current_token_ptr);

            if result != 0 {
                let result = GetTokenInformation(
                    current_token_ptr,
                    TokenElevation,
                    token_elevation_type_ptr as LPVOID,
                    mem::size_of::<winapi::um::winnt::TOKEN_ELEVATION_TYPE>() as u32,
                    &mut size,
                );
                if result != 0 {
                    return token_elevation.TokenIsElevated != 0;
                }
            }
        }
        false
    }

    /// Prompting the user with a graphical OS dialog for the root password,
    /// excuting the command with escalated privileges, and return the output
    ///
    /// On Windows, according to https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew#return-value,
    /// Output.status.code() shoudl be greater than 32 if the function succeeds,
    /// otherwise the value indicates the cause of the failure
    ///
    /// On Windows, Output.stdout and Output.stderr will always be empty as of now
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use elevated_command::Command;
    /// use std::process::Command as StdCommand;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut cmd = StdCommand::new("path to the application");
    ///     let elevated_cmd = Command::new(cmd);
    ///     let output = elevated_cmd.output().unwrap();
    /// }
    /// ```
    pub fn output(&mut self) -> Result<Output> {
        self.spawn()?;
        Ok(self.cmd.output()?)
    }

    pub fn spawn(&self) -> Result<Child> {
        let args = self
            .cmd
            .get_args()
            .map(|c| c.to_str().unwrap().to_string())
            .collect::<Vec<String>>();

        let stdout_file = NamedTempFile::new()?;
        let stderr_file = NamedTempFile::new()?;

        let stdout_path = stdout_file.path().to_string_lossy().to_string();
        let stderr_path = stderr_file.path().to_string_lossy().to_string();

        let mut parameters = format!(" > \"{}\" 2> \"{}\"", stdout_path, stderr_path);
        if !args.is_empty() {
            parameters = format!(" {} {}", args.join(" "), parameters);
        }

        // according to https://stackoverflow.com/a/38034535
        // the cwd always point to %SystemRoot%\System32 and cannot be changed by settting lpdirectory param
        let r = unsafe {
            ShellExecuteW(
                HWND(0),
                w!("runas"),
                &HSTRING::from(self.cmd.get_program()),
                &HSTRING::from(parameters),
                PCWSTR::null(),
                SW_HIDE,
            )
        };

        Ok(Output {
            status: ExitStatus::from_raw(r.0 as u32),
            stdout: Vec::<u8>::new(),
            stderr: Vec::<u8>::new(),
        })
    }
}
