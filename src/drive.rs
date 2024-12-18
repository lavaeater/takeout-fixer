use std::env;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl};
use oauth2::reqwest::async_http_client;
use tokio::net::TcpListener;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use url::Url;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use google_drive::{Client};
use google_drive::traits::FileOps;
use google_drive::types::File;
use crate::TAKEOUT_FOLDER_ID;
use anyhow::Result;

const REDIRECT_URI: &str = "http://localhost:8383";


#[derive(Serialize, Deserialize, Debug)]
pub struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: Option<u64>, // Optional timestamp for access token expiration
}

pub async fn login_google() -> anyhow::Result<Tokens> {
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
    let (authorize_url, _csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        // This example is requesting access to the "calendar" features and the user's profile.
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/drive".to_string(),
        ))
        .set_pkce_challenge(pkce_code_challenge)
        .url();
    if let Err(err) = open::that(authorize_url.as_str()) {
        eprintln!("Failed to open browser: {}", err);
    } 
    // A very naive implementation of the redirect server.
    let listener = TcpListener::bind("127.0.0.1:8383").await?;
    let code;
    let _state;
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

            _state = url
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
    // Exchange the code with a token.
    let token_response = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_code_verifier)
        .request_async(&async_http_client)
        .await;

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

async fn ensure_tokens() -> anyhow::Result<Tokens> {
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

pub async fn get_drive_client() -> Result<Client> {
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

pub const FOLDER_QUERY: &str = "mimeType = 'application/vnd.google-apps.folder'";

pub async fn list_google_drive(_folder: Option<String>) -> Result<Vec<File>> {
    let google_drive = get_drive_client()
        .await
        .expect("Failed to get Google Drive client");
    let file_client = google_drive.files();
    let response = file_client
        .list_all(
            "user", "", false, "",
            false, "name", format!("'{}' in parents", TAKEOUT_FOLDER_ID).as_str(),
            "", true, false, "")
        .await?;
    Ok(response.body)
}

pub async fn download_file(file_id: String) -> anyhow::Result<()> {
    let google_drive = get_drive_client()
        .await
        .expect("Failed to get Google Drive client");
    let file_client = google_drive.files();
    let file_name = file_client.get(&file_id, false, "", false, false).await?.body.name;
    let home_dir = dirs::home_dir().expect("Could not determine home directory");
    let home_dir = home_dir.join(file_name);
    let file = file_client.download_by_id(&file_id)
        .await.expect("Failed to get file");
    tokio::fs::write(home_dir, file.body).await?;
    Ok(())
}

fn get_token_file_path() -> PathBuf {
    let home_dir = dirs::home_dir().expect("Could not determine home directory");
    home_dir.join(".config/takeout-fixer/tokens.json")
}

async fn save_tokens(tokens: &Tokens) -> anyhow::Result<()> {
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

async fn load_tokens() -> anyhow::Result<Tokens> {
    let token_file_path = get_token_file_path();

    // Read and deserialize the token file
    let tokens_json = tokio::fs::read_to_string(token_file_path).await?;
    let tokens: Tokens = serde_json::from_str(&tokens_json)?;

    Ok(tokens)
}

async fn refresh_access_token(refresh_token: &str) -> anyhow::Result<Tokens> {
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