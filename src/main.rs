use clap::Parser;

fn main() {
    let cli = gild::cli::Cli::parse();
    if let Err(e) = gild::cli::run(cli) {
        eprintln!("Error: {e:?}");
        std::process::exit(2);
    }
}
