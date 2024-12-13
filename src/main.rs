use clap::{Parser, Subcommand};
use std::io::Read;
use google_drive::Client;

/// My CLI Tool
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The main command to run
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Prints a greeting message
    Greet {
        /// Name of the person to greet
        #[arg(short, long)]
        name: String,
    },
    /// Compares two directories
    Compare {
        /// Path to the first directory
        path_a: String,
        /// Path to the second directory
        path_b: String,
    },
    Takeout {
        google_drive_folder: String,
        output_folder: String,
    },
    ListGoogleDrive {
        google_drive_folder: String,
    },
    Test
    
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Greet { name } => {
            println!("Hello, {}!", name);
        }
        Commands::Compare { path_a, path_b } => {
            println!("Comparing directories:");
            println!("Path A: {}", path_a);
            println!("Path B: {}", path_b);
            // Add comparison logic here
        }
        Commands::Takeout { google_drive_folder, output_folder } => {
            
        }
        Commands::ListGoogleDrive { .. } => {}
        Commands::Test => {
            get_consent_url();
        }
    }
}
/*
{"state"=>"f19ea489-ea80-460a-905e-f4259227cc13", "code"=>"4/0AanRRrsbNdQnzqNqBYiluRBGkY6OiIbutw1goIG7VgS23ypELRuvS_ztJCNvaGLx1R3gBA", "scope"=>"https://www.googleapis.com/auth/drive", "controller"=>"supervisor/crm_integration", "action"=>"drive"}
 */


fn get_consent_url() {
    let mut google_drive = Client::new(
        "CLIENT_ID",
        "CLIENT_SECRET",
        "REDIRECT_URI",
        "",
        ""
    );

    // Get the URL to request consent from the user.
    // You can optionally pass in scopes. If none are provided, then the
    // resulting URL will not have any scopes.
    let user_consent_url = google_drive.user_consent_url(&["https://www.googleapis.com/auth/drive".to_string()]);
    println!("Go to this URL to get a code: {}", user_consent_url);
    // // In your redirect URL capture the code sent and our state.
    // // Send it along to the request for the token.
    // let code = "thing-from-redirect-url";
    // let state = "state-from-redirect-url";
    // let mut access_token = google_drive.get_access_token(code, state).await.unwrap();
    // 
    // // You can additionally refresh the access token with the following.
    // // You must have a refresh token to be able to call this function.
    // access_token = google_drive.refresh_access_token().await.unwrap();
}

fn read_a_file_or_something() {
    let path = ".";
    let entries = std::fs::read_dir(path).unwrap();
    for entry in entries {
        match entry {
            Ok(entry) => {
                println!("Processing entry: {:?}", entry);
                let file = std::fs::File::open(entry.path());
                match file {
                    Ok(mut file) => {
                        let mut buffer = Vec::new();
                        let content = file.read_to_end(&mut buffer);
                        match content {
                            Ok(sz) => {
                                println!("  got {} bytes", sz);
                                // we should work with buffer here
                            }
                            Err(e) => {
                                println!("  read error: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  open error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("  entry error: {:?}", e);
            }
        }
    }
}
