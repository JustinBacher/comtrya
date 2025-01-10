/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Luis Liu. All rights reserved.
 *  Licensed under the MIT License. See License in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

use super::Command;
use anyhow::{anyhow, Result};
use std::env;
use std::ffi::OsStr;
use std::process::{Child, Command as StdCommand, Output, Stdio};
use tracing::debug;
use which::which;

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
        let uid = unsafe { libc::getuid() };
        uid == 0
    }

    /// Prompting the user with a graphical OS dialog for the root password,
    /// excuting the command with escalated privileges, and return the output
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

    pub fn spawn(&mut self) -> Result<Child> {
        tracing::debug!(
            "Command: {} {}",
            &self.cmd.get_program().to_string_lossy(),
            &self
                .cmd
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect::<Vec<String>>()
                .join(" ")
        );

        let Ok(child) = self.cmd.spawn() else {
            debug!("Failed to spawn command inner");
            return Err(anyhow!("Failed to spawn command inner"));
        };

        Ok(child)
    }

    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.cmd.arg(arg.as_ref());
        self
    }

    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.cmd.stdin(cfg.into());
        self
    }

    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.cmd.stdout(cfg.into());
        self
    }

    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.cmd.stderr(cfg.into());
        self
    }
}
