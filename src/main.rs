use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use google_drive::types::File;
use google_drive::Client;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use google_drive::traits::FileOps;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use url::Url;

const REDIRECT_URI: &str = "http://localhost:8383";
const TAKEOUT_FOLDER_ID: &str = "1M2IDkPkChp8nBisf18-p_2-ZhG-nFSIhk68Acy8GQIlEIlrCb6XAGDc0Ty30MEoQDr-JHu1m";

#[derive(Serialize, Deserialize, Debug)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: Option<u64>, // Optional timestamp for access token expiration
}

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
    Takeout {
        google_drive_folder: String,
        output_folder: String,
    },
    List {
        google_drive_folder: Option<String>,
    },
    DownloadFile {
        file_id: String
    }
}

#[tokio::main]
async fn main() {
    //Not sure if it has to be done this way.
    tokio::task::spawn_blocking(|| {
        dotenv().ok();
    })
    .await
    .unwrap();

    let cli = Cli::parse();

    match cli.command {
        Commands::Takeout {
            google_drive_folder,
            output_folder,
        } => {}
        Commands::List { google_drive_folder } => {
            list_google_drive(None)
                .await
                .expect("Failed to list Google Drive files");
        }
        Commands::DownloadFile { file_id } => {
            download_file(file_id).await.expect("Failed to download file");
        }
    }
}
/*
{"state"=>"f19ea489-ea80-460a-905e-f4259227cc13", "code"=>"4/0AanRRrsbNdQnzqNqBYiluRBGkY6OiIbutw1goIG7VgS23ypELRuvS_ztJCNvaGLx1R3gBA", "scope"=>"https://www.googleapis.com/auth/drive", "controller"=>"supervisor/crm_integration", "action"=>"drive"}
 */

async fn login_google() -> Result<Tokens> {
    let google_client_id = ClientId::new(
        env::var("GOOGLE_CLIENT_ID").expect("Missing the GOOGLE_CLIENT_ID environment variable."),
    );
    let google_client_secret = ClientSecret::new(
        env::var("GOOGLE_CLIENT_SECRET")
            .expect("Missing the GOOGLE_CLIENT_SECRET environment variable."),
    );
    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .expect("Invalid authorization endpoint URL");
    let token_url = TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
        .expect("Invalid token endpoint URL");

    // Set up the config for the Google OAuth2 process.
    let client = BasicClient::new(
        google_client_id,
        Some(google_client_secret),
        auth_url,
        Some(token_url),
    )
    // This example will be running its own server at localhost:8080.
    // See below for the server implementation.
    .set_redirect_uri(
        RedirectUrl::new("http://localhost:8383".to_string()).expect("Invalid redirect URL"),
    );

    // Google supports Proof Key for Code Exchange (PKCE - https://oauth.net/2/pkce/).
    // Create a PKCE code verifier and SHA-256 encode it as a code challenge.
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL to which we'll redirect the user.
    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        // This example is requesting access to the "calendar" features and the user's profile.
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/drive".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    println!("Opening this URL in your browser:\n{authorize_url}\n");
    if let Err(err) = open::that(authorize_url.as_str()) {
        eprintln!("Failed to open browser: {}", err);
    } else {
        println!("Opened URL in the default browser!");
    }
    // A very naive implementation of the redirect server.
    let listener = TcpListener::bind("127.0.0.1:8383").await?;
    let code;
    let state;
    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let mut reader = BufReader::new(&mut stream);

            let mut request_line = String::new();
            reader.read_line(&mut request_line).await?;

            let redirect_url = request_line.split_whitespace().nth(1).unwrap();
            let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();
            code = url
                .query_pairs()
                .find(|(key, _)| key == "code")
                .map(|(_, code)| AuthorizationCode::new(code.into_owned()))
                .unwrap();

            state = url
                .query_pairs()
                .find(|(key, _)| key == "state")
                .map(|(_, state)| CsrfToken::new(state.into_owned()))
                .expect("State not found");

            let message = "Go back to your terminal :)";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                message.len(),
                message
            );
            stream.write_all(response.as_bytes()).await?;

            break;
        }
    }
    println!("Google returned the following code:\n{}\n", code.secret());
    println!(
        "Google returned the following state:\n{} (expected `{}`)\n",
        state.secret(),
        csrf_state.secret()
    );

    // Exchange the code with a token.
    let token_response = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(&async_http_client)
        .await;

    println!("Google returned the following token:\n{token_response:?}\n");
    match token_response {
        Ok(token) => {
            let tokens = Tokens {
                access_token: token.access_token().secret().to_string(),
                refresh_token: token
                    .refresh_token()
                    .map(|t| t.secret().to_string())
                    .unwrap_or_else(|| "".to_string()),
                expires_at: token
                    .expires_in()
                    .map(|duration| (chrono::Utc::now() + duration).timestamp() as u64),
            };
            save_tokens(&tokens).await.expect("Failed to save tokens");
            Ok(tokens)
        }
        Err(err) => {
            eprintln!("Error retrieving access token: {:?}", err);
            Err(err.into())
        }
    }
}

async fn ensure_tokens() -> Result<Tokens> {
    if let Ok(tokens) = load_tokens().await {
        if let Some(expires_at) = tokens.expires_at {
            if expires_at > chrono::Utc::now().timestamp() as u64 {
                return Ok(tokens);
            }
        }
        refresh_access_token(&tokens.refresh_token).await
    } else {
        login_google().await
    }
}

async fn get_drive_client() -> Result<Client> {
    let client_id =
        env::var("GOOGLE_CLIENT_ID").expect("Missing the GOOGLE_CLIENT_ID environment variable.");
    let client_secret = env::var("GOOGLE_CLIENT_SECRET")
        .expect("Missing the GOOGLE_CLIENT_SECRET environment variable.");
    let tokens = ensure_tokens().await?;
    Ok(Client::new(
        client_id,
        client_secret,
        REDIRECT_URI,
        tokens.access_token,
        tokens.refresh_token,
    ))
}

const FOLDER_QUERY: &str = "mimeType = 'application/vnd.google-apps.folder'";

async fn list_google_drive(folder: Option<String>) -> Result<()> {
    let google_drive = get_drive_client()
        .await
        .expect("Failed to get Google Drive client");
    let file_client = google_drive.files();
    let files = file_client
        .list_all(
            "user", "", false, "",
            false, "name", format!("'{}' in parents", TAKEOUT_FOLDER_ID).as_str(),
            "",true,false,"")
        .await
        .expect("Failed to list files");

    for file in files.body {
        println!("{}, {}", file.name, file.id);
    }
    Ok(())
}

async fn download_file(file_id: String) -> Result<()> {
    let google_drive = get_drive_client()
        .await
        .expect("Failed to get Google Drive client");
    let file_client = google_drive.files();
    let file_name = file_client.get(&file_id,false, "", false, false).await?.body.name;
    let home_dir = dirs::home_dir().expect("Could not determine home directory");
    let home_dir = home_dir.join(file_name);
    println!("Downloading file to {}", home_dir.display());
    let file = file_client.download_by_id(&file_id) 
        .await.expect("Failed to get file");
    tokio::fs::write(home_dir, file.body).await?;
    Ok(())
}

fn get_token_file_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Could not determine home directory");
    home_dir.join(".config/takeout-fixer/tokens.json")
}

async fn save_tokens(tokens: &Tokens) -> Result<()> {
    let token_file_path = get_token_file_path();

    // Ensure the directory exists
    if let Some(parent) = token_file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Serialize the tokens to JSON and write to file
    let tokens_json = serde_json::to_string(tokens)?;
    tokio::fs::write(token_file_path, tokens_json).await?;

    Ok(())
}

async fn load_tokens() -> Result<Tokens> {
    let token_file_path = get_token_file_path();

    // Read and deserialize the token file
    let tokens_json = tokio::fs::read_to_string(token_file_path).await?;
    let tokens: Tokens = serde_json::from_str(&tokens_json)?;

    Ok(tokens)
}

async fn refresh_access_token(refresh_token: &str) -> Result<Tokens> {
    let client_id = env::var("GOOGLE_CLIENT_ID")?;
    let client_secret = env::var("GOOGLE_CLIENT_SECRET")?;
    let client = BasicClient::new(
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
        AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    );

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(async_http_client)
        .await?;

    let new_tokens = Tokens {
        access_token: token_result.access_token().secret().to_string(),
        refresh_token: token_result
            .refresh_token()
            .map(|t| t.secret().to_string())
            .unwrap_or(refresh_token.to_string()),
        expires_at: token_result
            .expires_in()
            .map(|dur| (chrono::Utc::now() + dur).timestamp() as u64),
    };

    save_tokens(&new_tokens).await?;

    Ok(new_tokens)
}
