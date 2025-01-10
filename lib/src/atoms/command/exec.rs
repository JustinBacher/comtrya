use crate::atoms::Outcome;

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;

use super::super::Atom;
use crate::elevated_command::{Child, Command as ElevatedCommand};
use crate::utilities;
use anyhow::{anyhow, Context, Result};
use tracing::{debug, debug_span, error, info, span, trace, warn, Level};
use tracing_indicatif::{span_ext::IndicatifSpanExt, suspend_tracing_indicatif};

#[derive(Default)]
pub struct Exec {
    pub command: String,
    pub arguments: Vec<String>,
    pub working_dir: Option<String>,
    pub environment: Vec<(String, String)>,
    pub privileged: bool,
    pub privilege_provider: String,
    pub(crate) status: ExecStatus,
}

#[derive(Default)]
pub(crate) struct ExecStatus {
    code: i32,
    stdout: String,
    stderr: String,
}

#[allow(dead_code)]
pub fn new_run_command(command: String) -> Exec {
    Exec {
        command,
        ..Default::default()
    }
}

fn classify_and_log_message(line: &str) {
    let little_line = line.to_ascii_lowercase();
    if little_line.contains("error") {
        error!("{}", line);
    } else if little_line.contains("warn") {
        warn!("{}", line);
    } else {
        info!("{}", line);
    }
}

impl Exec {
    #[allow(dead_code)]
    fn elevate_if_required(&self) -> (String, Vec<String>) {
        // Depending on the priviledged flag and who who the current user is
        // we can determine if we need to prepend sudo to the command

        let privilege_provider = self.privilege_provider.clone();

        match (self.privileged, whoami::username().as_str()) {
            // Hasn't requested priviledged, so never try to elevate
            (false, _) => (self.command.clone(), self.arguments.clone()),

            // Requested priviledged, but is already root
            (true, "root") => (self.command.clone(), self.arguments.clone()),

            // Requested priviledged, but is not root
            (true, _) => (
                privilege_provider,
                [vec![self.command.clone()], self.arguments.clone()].concat(),
            ),
        }
    }

    fn start_command(&mut self) -> Result<Child> {
        let cmd = utilities::get_binary_path(&self.command)
            .map_err(|_| anyhow!("Command `{}` not found in path", self.command))?;

        let mut command = Command::new(&cmd);
        command
            .envs(self.environment.clone())
            .args(&self.arguments)
            .current_dir(self.working_dir.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|current_dir| current_dir.display().to_string())
                    .expect("Failed to get current directory")
            }));

        let child = match self.privileged {
            true => match self.elevate(command) {
                Ok(mut cmd) => cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn(),
                Err(e) => {
                    debug!("Unable to spawn command. {e}");
                    Err(anyhow!(e))
                }
            },
            false => command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    debug!("Unable to spawn command. {e}");
                    anyhow!("{e}")
                }),
        };

        debug!("Child: {child:?}");
        child
    }

    fn elevate(&mut self, cmd: Command) -> Result<ElevatedCommand> {
        let mut cmd = ElevatedCommand::from(cmd);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        Ok(cmd)
    }
}

impl std::fmt::Display for Exec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CommandExec with: privileged={}: {} {}",
            self.privileged,
            self.command,
            self.arguments.join(" ")
        )
    }
}

impl Atom for Exec {
    fn plan(&self) -> anyhow::Result<Outcome> {
        Ok(Outcome {
            // Commands may have side-effects, but none that can be "known"
            // without some sandboxed operations to detect filesystem and network
            // affects.
            // Maybe we'll look into this one day?
            side_effects: vec![],
            // Commands should always run, we have no cache-key based
            // determinism atm the moment.
            should_run: true,
        })
    }

    fn execute(&mut self) -> anyhow::Result<()> {
        let span = debug_span!("package.install", provider = self.command);
        span.pb_start();

        let mut child = self.start_command()?;
        debug!("Id: {}", child.id());
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        debug!("Spawned command");

        debug!("Spawning read thread");
        let out_handle = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Err(e) => {
                        error!("{}", e);
                    }
                    Ok(line) => {
                        classify_and_log_message(&line);
                    }
                }
            }
        });

        let err_handle = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Err(e) => {
                        error!("{}", e);
                    }
                    Ok(line) => {
                        classify_and_log_message(&line);
                    }
                }
            }
        });

        out_handle.join().ok();
        err_handle.join().ok();

        Ok(())
    }

    fn output_string(&self) -> String {
        self.status.stdout.clone()
    }

    fn error_message(&self) -> String {
        self.status.stderr.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::privilege::Privilege;
    use pretty_assertions::assert_eq;

    #[test]
    fn defaults() {
        let command_run = Exec {
            ..Default::default()
        };

        assert_eq!(String::from(""), command_run.command);
        assert_eq!(0, command_run.arguments.len());
        assert_eq!(None, command_run.working_dir);
        assert_eq!(0, command_run.environment.len());
        assert_eq!(false, command_run.privileged);

        let command_run = new_run_command(String::from("echo"));

        assert_eq!(String::from("echo"), command_run.command);
        assert_eq!(0, command_run.arguments.len());
        assert_eq!(None, command_run.working_dir);
        assert_eq!(0, command_run.environment.len());
        assert_eq!(false, command_run.privileged);
    }

    #[test]
    fn elevate() {
        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("echo"), command);
        assert_eq!(vec![String::from("Hello, world!")], args);

        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        command_run.privileged = true;
        command_run.privilege_provider = Privilege::Sudo.to_string();
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("sudo"), command);
        assert_eq!(
            vec![String::from("echo"), String::from("Hello, world!")],
            args
        );
    }

    #[test]
    fn elevate_doas() {
        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("echo"), command);
        assert_eq!(vec![String::from("Hello, world!")], args);

        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        command_run.privileged = true;
        command_run.privilege_provider = Privilege::Doas.to_string();
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("doas"), command);
        assert_eq!(
            vec![String::from("echo"), String::from("Hello, world!")],
            args
        );
    }
    #[test]
    fn elevate_run0() {
        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("echo"), command);
        assert_eq!(vec![String::from("Hello, world!")], args);

        let mut command_run = new_run_command(String::from("echo"));
        command_run.arguments = vec![String::from("Hello, world!")];
        command_run.privileged = true;
        command_run.privilege_provider = Privilege::Run0.to_string();
        let (command, args) = command_run.elevate_if_required();

        assert_eq!(String::from("run0"), command);
        assert_eq!(
            vec![String::from("echo"), String::from("Hello, world!")],
            args
        );
    }

    #[test]
    fn error_propagation() {
        let mut command_run = new_run_command(String::from("non-existant-command"));
        command_run.execute().expect_err("Command should fail");
    }
}
