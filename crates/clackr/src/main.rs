//! clackr — a MonkeyType-style typing test for your terminal.

mod app;
mod cli;
mod config_io;
mod term;
mod ui;

use clap::Parser;
use color_eyre::eyre::Result;

use crate::app::App;
use crate::cli::Cli;
use crate::term::caps::Caps;

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();

    let mut config = config_io::load(args.config.as_deref())?;
    args.apply_to(&mut config);

    // The informational flags all print and exit without touching the terminal.
    if args.config_path {
        match config_io::config_path() {
            Some(p) => println!("{}", p.display()),
            None => println!("(no config directory available on this platform)"),
        }
        return Ok(());
    }
    if args.list_languages || args.list_themes {
        list_assets(&args);
        return Ok(());
    }

    // Restoration is wired up before anything modifies the terminal, so even a
    // panic during setup leaves a usable shell behind.
    term::install_panic_hook();

    let result = ratatui::run(|terminal| -> Result<()> {
        let _guard = term::TermGuard::install(config.ui.caret);

        let size = terminal.size()?;
        let mut caps = Caps::detect(size.width, size.height);
        if args.ascii {
            caps.downgrade_to_ascii();
        }

        let rx = term::input::spawn();
        App::new(config, caps)
            .persisting_to(args.config.clone())
            .run(terminal, &rx)
    });

    // ratatui::run restores on its own, but a second restore is harmless and
    // covers the case where the closure returned early past the guard.
    term::restore();
    result
}

fn list_assets(args: &Cli) {
    if args.list_languages {
        for name in clackr_core::words::language_names() {
            println!("{name}");
        }
    }
    if args.list_themes {
        for name in clackr_core::words::theme_names() {
            println!("{name}");
        }
    }
}
