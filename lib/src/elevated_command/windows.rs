/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Luis Liu. All rights reserved.
 *  Licensed under the MIT License. See License in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

use super::Command;

use anyhow::Result;
use which::which;

use std::{
    fs::File,
    ffi::{OsStr, OsString},
    io::BufReader,
    os::windows::{ffi::OsStrExt, io::FromRawHandle},
    process::{Output, Stdio},
    mem,
};
use winapi::{
    shared::minwindef::{DWORD, LPVOID},
    um::{
        processthreadsapi::{GetCurrentProcess, OpenProcessToken},
        securitybaseapi::GetTokenInformation,
        winnt::{TokenElevation, HANDLE, TOKEN_ELEVATION, TOKEN_QUERY},
    }
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::PIPE_ACCESS_DUPLEX,
        System::{
            Pipes::{
                CreateNamedPipeW, PIPE_READMODE_BYTE,
                PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                WaitForSingleObject, INFINITE, STARTUPINFOW,
            },
        },
        UI::{
            Shell::{
                ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS,
                SEE_MASK_UNICODE, SHELLEXECUTEINFOW,
            },
            WindowsAndMessaging::SW_HIDE
        }
    },
};

pub struct Child {
    pub stdin: Option<File>,
    pub stdout: Option<BufReader<File>>,
    pub stderr: Option<BufReader<File>>,
}

pub struct ChildStdWriter(File);

impl ChildStdWriter {
    pub fn take(self) -> File {
        self.0
    }
}

pub struct ChildStdReader(BufReader<File>);

impl ChildStdReader {
    pub fn take(self) -> BufReader<File> {
        self.0
    }
}

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
        // TODO: actually provide output parody of std Command
        Ok(self.cmd.output()?)
    }

    // Unfortunately this is the only way to elevate a sub-process on
    // windows while still being able to pipe the stdin/out/err in realtime
    pub fn spawn(&self) -> Result<Child> {
        let mut startup_information = STARTUPINFOW::default();
        startup_information.cb = size_of::<STARTUPINFOW>() as u32;

        let security_attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            bInheritHandle: true.into(),
            lpSecurityDescriptor: std::ptr::null_mut(),
        };

        let args = format!(
            "{}",
            self.cmd
                .get_args()
                .map(|a| a.to_str().unwrap().to_string())
                .collect::<Vec<String>>().join(" ")
        );

        let stdin_pipe = unsafe { CreateNamedPipeW(
            w!(r"\\.\pipe\elevated_stdin"),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            4096,
            4096,
            0,
            Some(&security_attributes),
         )};

        let stdout_pipe = unsafe { CreateNamedPipeW(
            w!(r"\\.\pipe\elevated_stdout"),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            4096,
            4096,
            0,
            Some(&security_attributes),
        )};

        let stderr_pipe = unsafe { CreateNamedPipeW(
            w!(r"\\.\pipe\elevated_stderr"),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            4096,
            4096,
            0,
            Some(&security_attributes),
        )};
        
        let args = format!("{} < \\\\.\\pipe\\elevated_stdin > \\\\.\\pipe\\elevated_stdout 2> \\\\.\\pipe\\elevated_stderr", args);
        let args_wide: Vec<u16> = OsString::from(args).encode_wide().chain(Some(0)).collect();


        let program = which(self.cmd.get_program())?
            .to_string_lossy()
            .to_string();
        
        let program_wide: Vec<u16> = OsString::from(program)
        .encode_wide()
        .chain(Some(0))
        .collect();

        let mut shell_info = SHELLEXECUTEINFOW {
            cbSize: mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_UNICODE,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(program_wide.as_ptr()),
            lpParameters: PCWSTR(args_wide.as_ptr()), 
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        
        let _success = unsafe { ShellExecuteExW(&mut shell_info) }?;
        
        let stdin_writer = unsafe { std::fs::File::from_raw_handle(stdin_pipe.0 as *mut _) };
        let stdout_reader = BufReader::new(
            unsafe { File::from_raw_handle(stdout_pipe.0 as *mut _) }
        );
        let stderr_reader = BufReader::new(
            unsafe { File::from_raw_handle(stderr_pipe.0 as *mut _) }
        );
        
        // let mut stdout_lines = stdout_reader.lines();
        // let mut stderr_lines = stderr_reader.lines();
        //
        // thread::spawn(move || {
        //     loop {
        //         match (stdout_lines.next(), stderr_lines.next()) {
        //             (None, None) => break,
        //             (Some(Ok(line)), _) => println!("stdout: {}", line),
        //             (_, Some(Ok(line))) => println!("stderr: {}", line),
        //             (Some(Ok(out_line)), Some(Ok(err_line))) => {
        //                 println!("stdout: {}", out_line);
        //                 println!("stderr: {}", err_line);
        //             },
        //             _ => continue,
        //         }
        //     }
        // });

        unsafe { WaitForSingleObject(shell_info.hProcess, INFINITE) };

        Ok(Child {
            stdin: Some(stdin_writer),
            stdout: Some(stdout_reader),
            stderr: Some(stderr_reader),
        })
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.cmd.arg(arg.as_ref());
        self
    }

    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.stdin = Some(cfg.into());
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.stdout = Some(cfg.into());
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.stderr = Some(cfg.into());
        self
    }
}
