use colored::*;

pub fn print_banner() {
    println!(
        "{}",
        "================================================================================".yellow()
    );
    println!(
        "{}",
        r#"
  ___ ___ .__               ____  __.                          .__   
 /   |   \|__|__  __ ____  |    |/ _|___________  ____   ____ |  |  
/    ~    \  |\ \/ // __ \ |      < _/ __ \_  __ \/    \_/ __ \|  |  
\    Y    /  | \  /\  ___/ |    |  \\  ___/|  | \/   |  \  ___/|  |__
 \___|_  /|__|  \/  \___  >|____|__ \\___  >__|  |___|  /\___  >____/
       \/               \/         \/    \/           \/     \/      
"#
        .bright_yellow()
        .bold()
    );
    println!(
        "  {}",
        "🐝 Autonomous P2P Agent Subcontracting & Optimistic Settlement Engine in Rust"
            .cyan()
            .bold()
    );
    println!(
        "  {}",
        "⚡ Built for Swarm Village | Powered by HERŌ Network & Web3Bridge".magenta()
    );
    println!(
        "  {}",
        "🦀 High-Performance Actor Protocol & Game-Theoretic Verification".green()
    );
    println!(
        "{}",
        "================================================================================".yellow()
    );
}

pub fn log_step(step: u8, title: &str, details: &str) {
    println!(
        "\n{} {} {}",
        format!("[Step {}]", step).bright_blue().bold(),
        title.bright_white().bold(),
        "▶".bright_cyan()
    );
    println!("  {}", details.dimmed());
}

pub fn log_event(source: &str, message: &str) {
    println!(
        "  {} {}",
        format!("[{}]", source).bright_purple().bold(),
        message
    );
}

pub fn log_success(message: &str) {
    println!(
        "  {} {}",
        "✔ [SUCCESS]".bright_green().bold(),
        message.bright_green()
    );
}

pub fn log_dispute(message: &str) {
    println!(
        "  {} {}",
        "✖ [DISPUTE / SLASH]".bright_red().bold(),
        message.bright_red()
    );
}
