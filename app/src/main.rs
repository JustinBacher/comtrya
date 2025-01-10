#[macro_use]
extern crate tracing_indicatif;

use crate::commands::ComtryaCommand;
use crate::config::{Commands, GlobalArgs};

use std::io;
use std::thread::sleep;
use std::time::Duration;

use comtrya_lib::contexts::build_contexts;
use comtrya_lib::contexts::Contexts;
use comtrya_lib::manifests;

use clap::Parser;
use indicatif::ProgressState;
use indicatif::ProgressStyle;
use tracing::instrument::WithSubscriber;
use tracing::{error, Instrument, Level, Metadata};
use tracing_core::span;
use tracing_core::{
    span::{Attributes, Id},
    Callsite, LevelFilter, Subscriber,
};
use tracing_indicatif::filter::hide_indicatif_span_fields;
use tracing_indicatif::filter::IndicatifFilter;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::layer::Filter;
use tracing_subscriber::layer::Layer;
use tracing_subscriber::registry::{LookupSpan, SpanData, SpanRef};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Registry;
use tracing_subscriber::{fmt::writer::MakeWriterExt, layer::SubscriberExt, FmtSubscriber};

mod commands;
mod config;
use config::Config;
use update_informer::registry;

#[derive(Debug)]
pub struct Runtime {
    pub(crate) args: GlobalArgs,
    pub(crate) config: Config,
    pub(crate) contexts: Contexts,
}

struct CompletionLayer;

pub(crate) fn execute(runtime: Runtime) -> anyhow::Result<()> {
    match &runtime.args.command {
        Commands::Apply(apply) => apply.execute(&runtime),
        Commands::Status(apply) => apply.status(&runtime),
        Commands::Version(version) => version.execute(&runtime),
        Commands::Contexts(contexts) => contexts.execute(&runtime),
        Commands::GenCompletions(gen_completions) => gen_completions.execute(&runtime),
    }
}
pub fn get_child_msgs(state: &ProgressState, w: &mut dyn std::fmt::Write) {
    let span_data = state.in_current_span();
    let _ = w.write_str(&format!(
        "{:?}",
        span_data
            .span()
            .field("msg")
            .map(|f| f.to_string())
            .unwrap_or_default()
    ));
}

fn configure_tracing(args: &GlobalArgs) {
    let stderr_writer = IndicatifLayer::<tracing_subscriber::Registry>::new().get_stderr_writer();

    let indicatif_layer = IndicatifLayer::new()
        .with_progress_style(
            ProgressStyle::with_template(
                "{span_child_prefix}{span_fields} -- {span_name} {wide_msg}}",
            )
            .expect("Unable to create style"), //.with_key("child_msg", get_child_msgs),
        )
        .with_span_field_formatter(hide_indicatif_span_fields(DefaultFields::new()));

    let registry = tracing_subscriber::registry().with(
        indicatif_layer
            .with_filter(IndicatifFilter::new(true))
            .with_filter(LevelFilter::from_level(Level::DEBUG)),
    );

    if args.verbose > 0 {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_ansi(true)
                    .with_line_number(false)
                    .with_thread_names(false)
                    .with_target(false)
                    .with_file(false)
                    .with_writer(stderr_writer)
                    .with_filter(LevelFilter::from_level(match args.verbose {
                        1 => Level::DEBUG,
                        2 => Level::ERROR,
                        _ => Level::TRACE,
                    })),
            )
            .init();
    } else {
        registry.init();
    }
}

fn main() -> anyhow::Result<()> {
    let args = GlobalArgs::parse();
    configure_tracing(&args);

    let config = match config::load_config(&args) {
        Ok(config) => config,
        Err(error) => {
            error!("{}", error.to_string());
            panic!();
        }
    };

    if !config.disable_update_check {
        check_for_updates(args.no_color);
    }

    // Run Context Providers
    let contexts = build_contexts(&config);
    let runtime = Runtime {
        args,
        config,
        contexts,
    };

    execute(runtime)?;

    Ok(())
}

fn check_for_updates(no_color: bool) {
    use colored::*;
    use update_informer::{registry, Check};

    if no_color {
        control::set_override(false);
    }

    let pkg_name = env!("CARGO_PKG_NAME");
    let pkg_version = env!("CARGO_PKG_VERSION");
    let informer = update_informer::new(registry::Crates, pkg_name, pkg_version);

    if let Some(new_version) = informer.check_version().ok().flatten() {
        let msg = format!(
            "A new version of {pkg_name} is available: v{pkg_version} -> {new_version}",
            pkg_name = pkg_name.italic().cyan(),
            new_version = new_version.to_string().green()
        );

        let release_url =
            format!("https://github.com/{pkg_name}/{pkg_name}/releases/tag/{new_version}").blue();
        let changelog = format!("Changelog: {release_url}",);

        let cmd = format!(
            "Run to update: {cmd}",
            cmd = "curl -fsSL https://get.comtrya.dev | sh".green()
        );

        println!("\n{msg}\n{changelog}\n{cmd}");
    }
}
