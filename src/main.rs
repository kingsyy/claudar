mod cli;
mod config_cmd;
mod history_cmd;
mod service;
mod setup;
mod status;
mod usage;

use clap::Parser;
use cli::{Cli, Commands, ConfigAction, InstancesAction};
use claudar_core::{config, monitor, notification_trait};
use notify_rust::Timeout;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::DEBUG.into()),
            )
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::Level::ERROR.into()),
            )
            .init();
    }

    match cli.command {
        Commands::Setup { instance } => {
            setup::run_setup(cli.verbose, instance)?;
        }
        Commands::Run => {
            monitor::run_monitor(true).await?;
        }
        Commands::Usage { instance } => {
            usage::run_usage(cli.verbose, instance).await?;
        }
        Commands::Status => {
            status::run_status(cli.verbose).await?;
        }
        Commands::Config { action } => match action {
            ConfigAction::List => {
                config_cmd::handle_config_list()?;
            }
            ConfigAction::Get { key } => {
                config_cmd::handle_config_get(&key)?;
            }
            ConfigAction::Set { key, value } => {
                config_cmd::handle_config_set(&key, &value)?;
            }
        },
        Commands::Instances { action } => match action {
            InstancesAction::List => {
                config_cmd::handle_instances_list()?;
            }
            InstancesAction::Add { name } => {
                config_cmd::handle_instances_add(&name)?;
            }
            InstancesAction::Remove { name } => {
                config_cmd::handle_instances_remove(&name)?;
            }
        },
        Commands::History {
            instance,
            view,
            days,
            json,
        } => {
            let cfg = config::Config::load()?;
            history_cmd::run_history(&cfg, instance, view, days, json)?;
        }
        Commands::SetupService => {
            service::install_service()?;
        }
        Commands::Start => {
            service::start_service()?;
        }
        Commands::Stop => {
            service::stop_service()?;
        }
        Commands::UninstallService => {
            service::uninstall_service()?;
        }
        Commands::TestNotification => {
            use notification_trait::{NotificationSender, RealNotificationSender};

            let cfg = config::Config::load()?;
            let sender = RealNotificationSender;
            let timeout = if cfg.notifications.persistent {
                Timeout::Never
            } else {
                Timeout::Milliseconds(10000)
            };
            let sound = cfg
                .notifications
                .sound
                .then_some(cfg.notifications.sound_name.as_str());

            println!(
                "Sending test notifications (sound: {}, persistent: {})...",
                cfg.notifications.sound, cfg.notifications.persistent
            );

            sender.send(
                "[TEST] ⚠️ Claude Usage Alert: 5-hour Limit",
                "You've used 75% of your 5-hour limit.\nResets in 2h 30m\nAt current pace: 95% of limit",
                timeout,
                sound,
            )?;
            println!("  ✓ Threshold alert");

            std::thread::sleep(std::time::Duration::from_millis(600));

            sender.send(
                "[TEST] 💡 Unused 7-day Capacity Warning",
                "Your 7-day limit resets in 5h 30m with 35% unused capacity.\nConsider using it for large tasks before it expires!\n(Your 5-hour limit has 80% remaining, resets 14:00 10-06-2026)",
                timeout,
                sound,
            )?;
            println!("  ✓ Capacity warning");

            std::thread::sleep(std::time::Duration::from_millis(600));

            sender.send(
                "[TEST] ✓ Claude 7-day Limit Reset",
                "Your 7-day usage limit has been reset.\nYou now have fresh capacity available.",
                timeout,
                sound,
            )?;
            println!("  ✓ Reset notification");

            println!("\nAll 3 test notifications sent — check your notification center.");
        }
    }

    Ok(())
}
