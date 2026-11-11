use crate::LockExt;
use crate::process::executable;
use serde::Serialize;
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Default, Serialize)]
pub struct AuthStatus {
    pub installed: bool,
    pub installing: bool,
    pub install_message: Option<String>,
    pub logged_in: bool,
    pub pending: bool,
    pub url: Option<String>,
    pub error: Option<String>,
}
#[derive(Default)]
pub struct AuthManager {
    pub install_guard: Mutex<()>,
    pub status: Arc<Mutex<AuthStatus>>,
    pub input: Arc<Mutex<Option<ChildStdin>>>,
    pub child: Arc<Mutex<Option<Arc<Mutex<Child>>>>>,
    generation: Arc<AtomicU64>,
    start_guard: Arc<Mutex<()>>,
}
fn command(bin: &str) -> Result<Command, String> {
    let mut cmd = Command::new(executable(bin)?);
    crate::process::cli_environment(&mut cmd);
    if std::path::Path::new(cmd.get_program()).starts_with(crate::model::engine_dir().join("claude-code")) {
        cmd.env("DISABLE_AUTOUPDATER", "1");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    Ok(cmd)
}
pub fn valid_login_url(url: &str) -> bool {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) { return false; }
    let Ok(url) = tauri::Url::parse(url) else { return false; };
    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        && url.port().is_none()
        && matches!(url.host_str(), Some("claude.ai" | "console.anthropic.com" | "claude.com" | "platform.claude.com"))
}
pub fn open_login(url: &str) -> Result<(), String> {
    if !valid_login_url(url) { return Err("URL di accesso Claude non valido.".into()); }
    #[cfg(windows)]
    {
        use windows::{core::{w, PCWSTR}, Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}};
        let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(url.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        if result.0 as isize <= 32 {
            return Err(format!("Windows non ha aperto il browser (codice {}).", result.0 as isize));
        }
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") { "/usr/bin/open" } else { "xdg-open" };
        if !Command::new(program).arg(url).status().map_err(|e| e.to_string())?.success() {
            return Err("Impossibile aprire il browser predefinito.".into());
        }
    }
    Ok(())
}
fn browser_helper_url(args: &[String]) -> Result<&str, String> {
    if args.len() != 2 || !valid_login_url(&args[1]) {
        return Err("URL di accesso Claude non valido.".into());
    }
    Ok(&args[1])
}
pub fn browser_helper(args: &[String]) -> Option<Result<(), String>> {
    if std::env::var_os("PARLO_CLAUDE_BROWSER_HELPER").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return None;
    }
    Some(browser_helper_url(args).and_then(open_login))
}
fn login_command(bin: &str) -> Result<Command, String> {
    let mut login = command(bin)?;
    #[cfg(windows)]
    {
        // Claude treats BROWSER as an executable path, not a shell command.
        // Re-enter only the URL helper, before Tauri/single-instance startup.
        login.env("BROWSER", std::env::current_exe().map_err(|e| e.to_string())?)
            .env("PARLO_CLAUDE_BROWSER_HELPER", "1");
    }
    login.args(["auth", "login"]);
    Ok(login)
}
fn login_url(line: &str) -> Option<String> {
    line.match_indices("https://").find_map(|(start, _)| {
        let url = line[start..].split(|c: char| c.is_whitespace() || c.is_control())
            .next().unwrap_or_default();
        valid_login_url(url).then(|| url.to_string())
    })
}
fn login_output(stdout: impl Read + Send + 'static, stderr: impl Read + Send + 'static) -> mpsc::Receiver<String> {
    let (send, receive) = mpsc::channel();
    let errors = send.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if send.send(line).is_err() { break; }
        }
    });
    // Process links from either stream, without saving login/account data to disk.
    thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if errors.send(line).is_err() { break; }
        }
    });
    receive
}
fn wait_for_login(
    child: &Mutex<Child>, receive: mpsc::Receiver<String>, generation: &AtomicU64,
    version: u64, link_timeout: Duration, mut on_url: impl FnMut(String),
) -> Result<(), &'static str> {
    let started = Instant::now();
    let mut found_url = false;
    loop {
        if generation.load(Ordering::SeqCst) != version { return Ok(()); }
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => if let Some(url) = login_url(&line) { found_url = true; on_url(url); },
            Err(mpsc::RecvTimeoutError::Disconnected) => thread::sleep(Duration::from_millis(100)),
            Err(mpsc::RecvTimeoutError::Timeout) => {},
        }
        if child.guard().try_wait().map_err(|_| "Impossibile verificare il processo di accesso.")?.is_some() { return Ok(()); }
        let error = if !found_url && started.elapsed() >= link_timeout {
            Some("Claude Code non ha fornito la pagina di accesso. Annulla e riprova; verifica la connessione a Internet.")
        } else if started.elapsed() >= Duration::from_secs(480) {
            Some("Tempo per l’accesso scaduto. Riprova e completa la pagina nel browser.")
        } else { None };
        if let Some(error) = error {
            let _ = child.guard().kill();
            let _ = child.guard().wait();
            return Err(error);
        }
    }
}
pub fn check(bin: &str) -> Result<AuthStatus, String> {
    let output = command(bin)?
        .args(["auth", "status"])
        .output()
        .map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "Impossibile verificare l’account Claude.".to_string())?;
    let logged_in = value["loggedIn"].as_bool()
        .ok_or("La versione di Claude Code non restituisce uno stato di accesso valido.")?;
    Ok(AuthStatus {
        installed: true,
        logged_in: output.status.success() && logged_in,
        ..Default::default()
    })
}
impl AuthManager {
    pub fn cancel(&self) {
        let _guard = self.start_guard.guard();
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.input.guard().take();
        if let Some(child) = self.child.guard().take() {
            let mut child = child.guard();
            if child.kill().is_ok() { let _ = child.wait(); }
        }
        *self.status.guard() = AuthStatus::default();
    }
    pub fn start(&self, app: AppHandle, bin: String, force: bool) -> Result<AuthStatus, String> {
        let _guard = self.start_guard.guard();
        let current = self.status.guard().clone();
        if current.pending {
            return Ok(current);
        }
        let checked = check(&bin)?;
        if checked.logged_in && !force {
            *self.status.guard() = checked.clone();
            return Ok(checked);
        }
        let mut child = login_command(&bin)?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        *self.input.guard() = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let receive = login_output(stdout, stderr);
        let child = Arc::new(Mutex::new(child));
        *self.child.guard() = Some(child.clone());
        let status = AuthStatus {
            installed: true,
            pending: true,
            ..Default::default()
        };
        *self.status.guard() = status.clone();
        let _ = app.emit("claude-auth-changed", status.clone());
        let state = self.status.clone();
        let input = self.input.clone();
        let holder = self.child.clone();
        let generation = self.generation.clone();
        let start_guard = self.start_guard.clone();
        let version = generation.fetch_add(1, Ordering::SeqCst) + 1;
        thread::spawn(move || {
            let completion = wait_for_login(&child, receive, &generation, version, Duration::from_secs(45), |url| {
                let _guard = start_guard.guard();
                if generation.load(Ordering::SeqCst) == version {
                    if state.guard().url.as_ref() != Some(&url) {
                        state.guard().url = Some(url);
                        let _ = app.emit("claude-auth-changed", state.guard().clone());
                    }
                }
            });
            let _guard = start_guard.guard();
            if generation.load(Ordering::SeqCst) != version {
                return;
            }
            input.guard().take();
            holder.guard().take();
            let result = check(&bin).unwrap_or_default();
            let logged = result.logged_in;
            *state.guard() = AuthStatus {
                error: if logged {
                    None
                } else {
                    Some(completion.err().unwrap_or("Accesso non completato. Puoi riprovare.").into())
                },
                ..result
            };
            let _ = app.emit("claude-auth-changed", state.guard().clone());
        });
        Ok(status)
    }
    pub fn code(&self, code: &str) -> Result<(), String> {
        if code.trim().is_empty() || code.len() > 4096 || code.contains(['\n', '\r']) {
            return Err("Codice di accesso non valido.".into());
        }
        let mut input = self.input.guard();
        let pipe = input.as_mut().ok_or("Nessun accesso in corso.")?;
        writeln!(pipe, "{}", code.trim()).map_err(|e| e.to_string())?;
        pipe.flush().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn browser_helper_accepts_only_one_validated_url_argument() {
        let url = "https://claude.com/cai/oauth/authorize?code=true&state=synthetic&scope=user%3Aprofile";
        assert_eq!(browser_helper_url(&["PARLO.exe".into(), url.into()]).unwrap(), url);
        for args in [
            vec!["app.exe".into()],
            vec!["app.exe".into(), "file:///C:/Windows".into()],
            vec!["app.exe".into(), "https://evil.test/".into()],
            vec!["app.exe".into(), url.into(), "extra".into()],
        ] { assert!(browser_helper_url(&args).is_err()); }
    }

    #[cfg(windows)]
    #[test]
    fn login_delegates_browser_to_native_helper_without_shell_arguments() {
        let login = login_command(std::env::current_exe().unwrap().to_str().unwrap()).unwrap();
        let browser = login.get_envs().find(|(key, _)| *key == "BROWSER").unwrap().1.unwrap();
        assert_eq!(browser, std::env::current_exe().unwrap().as_os_str());
        assert!(login.get_envs().any(|(key, value)| key == "PARLO_CLAUDE_BROWSER_HELPER" && value == Some(std::ffi::OsStr::new("1"))));
        assert_eq!(login.get_args().collect::<Vec<_>>(), ["auth", "login"]);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Opens a public Claude page in the user's default browser; run explicitly"]
    fn browser_opener_smoke() {
        // Uses the exact same opener as automatic login and the manual button.
        let args = vec!["PARLO.exe".into(), "https://claude.com/?parlo_browser_check=1&source=login#browser-check".into()];
        open_login(browser_helper_url(&args).unwrap()).unwrap();
    }

    #[test]
    fn login_links_accept_current_and_legacy_hosts() {
        for url in [
            "https://claude.com/cai/oauth/authorize?code=true&state=synthetic",
            "https://platform.claude.com/oauth/authorize?state=synthetic",
            "https://claude.ai/oauth/authorize?state=synthetic",
            "https://console.anthropic.com/oauth/authorize?state=synthetic",
        ] {
            assert!(valid_login_url(url));
            assert_eq!(login_url(&format!("If the browser didn't open, visit: {url}\u{1b}[0m")), Some(url.into()));
        }
        for url in [
            "https://claude.com.evil.test/oauth/authorize", "http://claude.com/cai/oauth/authorize",
            "https://claude.com@evil.test/", "https://user@claude.com/",
            "https://claude.com:8080/", "https://claude.com/\nother", "https://evil.test/",
        ] { assert!(!valid_login_url(url), "{url}"); }
    }

    #[test]
    fn login_reads_links_from_both_streams() {
        let output = login_output(
            Cursor::new(b"Opening browser\nhttps://claude.com/cai/oauth/authorize?state=synthetic\n"),
            Cursor::new(b"https://platform.claude.com/oauth/authorize?state=synthetic\n"),
        );
        let mut links = Vec::new();
        while let Ok(line) = output.recv_timeout(Duration::from_secs(2)) {
            if let Some(url) = login_url(&line) { links.push(url); }
        }
        links.sort();
        assert_eq!(links, ["https://claude.com/cai/oauth/authorize?state=synthetic", "https://platform.claude.com/oauth/authorize?state=synthetic"]);
    }

    #[cfg(windows)]
    #[test]
    fn login_without_link_times_out_even_after_output_closes() {
        use std::os::windows::process::CommandExt;
        let child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap();
        let child = Mutex::new(child);
        let started = Instant::now();
        let result = wait_for_login(&child, login_output(Cursor::new(b""), Cursor::new(b"")),
            &AtomicU64::new(1), 1, Duration::from_millis(20), |_| panic!("Unexpected link"));
        assert!(result.unwrap_err().contains("pagina di accesso"));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(child.guard().try_wait().unwrap().is_some());
    }
}
