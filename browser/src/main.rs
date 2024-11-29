use anyhow::Result;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use log::{error, info};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct UserCredentials {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BrowserSettings {
    start_url: String,
    incognito: bool,
    max_navigation_timeout: u64,
    allowed_domains: Vec<String>,
}

struct SecureBrowser {
    remote_server_url: String,
    http_client: Client,
    browser: Option<Browser>,
}

impl SecureBrowser {
    async fn new(server_url: &str) -> Result<Self> {
        Ok(Self {
            remote_server_url: server_url.to_string(),
            http_client: Client::new(),
            browser: None,
        })
    }

    async fn show_login_dialog(&self) -> Result<(Browser, Page), Box<dyn std::error::Error>> {
        let (browser, mut handler) = Browser::launch(BrowserConfig::builder().with_head().build()?).await?;

        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if let Err(e) = h {
                    error!("Browser handler error: {:?}", e);
                }
            }
        });

        let page = browser.new_page("about:blank").await?;

        let html = r#"
        <!DOCTYPE html>
<html>
<head>
    <title>Login</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            margin: 0;
            background-color: #f0f0f0;
        }
        .login-form {
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        input {
            display: block;
            margin: 10px 0;
            padding: 8px;
            width: 200px;
        }
        button {
            background: #4CAF50;
            color: white;
            border: none;
            padding: 10px;
            width: 100%;
            border-radius: 4px;
            cursor: pointer;
        }
        button:hover {
            background: #45a049;
        }
    </style>
</head>
<body>
    <div class="login-form">
        <h2>Login</h2>
        <form id="loginForm">
            <input type="text" id="username" placeholder="Username" required>
            <input type="password" id="password" placeholder="Password" required>
            <button type="submit">Login</button>
        </form>
    </div>
    <script>
        const form = document.getElementById('loginForm');
        form.addEventListener('submit', (e) => {
            e.preventDefault();
            const username = document.getElementById('username').value;
            const password = document.getElementById('password').value;
            window.credentials = { username, password };
        });
    </script>
</body>
</html>
        "#;

        page.set_content(html).await?;
        
        Ok((browser, page))
    }

    async fn get_credentials_from_page(&self, page: &Page) -> Result<UserCredentials> {
        page.evaluate(
            r#"
            new Promise((resolve) => {
                const form = document.getElementById('loginForm');
                form.addEventListener('submit', (e) => {
                    e.preventDefault();
                    const username = document.getElementById('username').value;
                    const password = document.getElementById('password').value;
                    resolve({ username, password });
                });
            })
            "#,
        )
        .await?;

        let credentials = page.evaluate("window.credentials").await?;
        let value = credentials.object().value.clone();
        let value = value.ok_or_else(|| anyhow::anyhow!("Credentials not found"))?;
        let credentials: UserCredentials = serde_json::from_value(value)?;
        Ok(credentials)
    }

    async fn authenticate(&self, credentials: &UserCredentials) -> Result<bool> {
        let response = self
            .http_client
            .post(&format!("{}/authenticate", self.remote_server_url))
            .json(credentials)
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => Ok(true),
            _ => Ok(false),
        }
    }

    async fn fetch_browser_settings(&self) -> Result<BrowserSettings> {
        let settings = self
            .http_client
            .get(&format!("{}/browser-settings", self.remote_server_url))
            .json(&());
        
        let settings = settings.send().await?.json::<BrowserSettings>().await?;

        Ok(settings)
    }

    async fn launch_secure_browser(
        &mut self,
        settings: &BrowserSettings,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = BrowserConfig::builder()
            .with_head()
            .build()?;

        let (browser, mut handler) = Browser::launch(config).await?;

        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if let Err(e) = h {
                    error!("Browser handler error: {:?}", e);
                }
            }
        });

        let page = browser.new_page("about:blank").await?;
        page.goto(&settings.start_url).await?;

        self.browser = Some(browser);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if let Some(mut browser) = self.browser.take() {
            browser.close().await?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Update server URL to use port 3000
    let mut secure_browser = SecureBrowser::new("http://localhost:8080").await?;
    
    let (mut login_browser, login_page) = secure_browser.show_login_dialog().await?;
    let credentials = secure_browser.get_credentials_from_page(&login_page).await?;
    login_browser.close().await?;

    // Try with hardcoded credentials from server
    if credentials.username != "admin_user" || credentials.password != "secure_password" {
        error!("Invalid credentials. Please use admin_user/secure_password");
        return Ok(());
    }

    if secure_browser.authenticate(&credentials).await? {
        info!("Authentication successful");

        let settings = secure_browser.fetch_browser_settings().await?;
        secure_browser.launch_secure_browser(&settings).await?;

        tokio::signal::ctrl_c().await?;
        secure_browser.close().await?;
    } else {
        error!("Authentication failed");
    }

    Ok(())
}