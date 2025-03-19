use std::io;

use clap::{Command, CommandFactory, Parser};
use clap_complete::{Generator, Shell, generate};

use super::ComtryaCommand;
use crate::{GlobalArgs, Runtime};

#[derive(Parser, Debug)]
#[command(arg_required_else_help = true)]
pub(crate) struct GenCompletions {
    /// If provided, outputs the completion file for given shell
    #[arg(value_enum)]
    shell: Shell,
}

fn print_completions<G: Generator>(generator: G, cmd: &mut Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}

impl ComtryaCommand for GenCompletions {
    fn execute(&self, _runtime: &Runtime) -> anyhow::Result<()> {
        print_completions(self.shell, &mut GlobalArgs::command());

        Ok(())
    }
}
