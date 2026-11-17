//! Mises à jour automatiques.
//!
//! Coquille lit `latest.json` sur la dernière release GitHub publiée, télécharge la nouvelle version
//! en arrière-plan (signature vérifiée par le plugin updater de Tauri), puis l'installe à un moment
//! où cela ne dérange pas : fenêtre fermée, aucune capture en cours et, sous Windows, deux minutes
//! sans activité clavier ou souris. Sans l'installation automatique, un bouton fait le travail.

use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::utils::config::BundleType;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const FIRST_CHECK: Duration = Duration::from_secs(20);
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);
const POLL: Duration = Duration::from_secs(30);
/// Windows : inactivité clavier et souris demandée avant une installation automatique
/// (l'installateur affiche une petite barre de progression et relance Coquille).
const IDLE_BEFORE_INSTALL: Duration = Duration::from_secs(120);
const MARKER: &str = "mise-a-jour.json";

/// Comment cette copie de Coquille peut se mettre à jour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Installation sans intervention : Windows, macOS, AppImage.
    Auto,
    /// Installation sur demande seulement : le paquet .deb ou .rpm demande le mot de passe administrateur.
    OnDemand,
    /// Coquille ne s'installe pas elle-même : paquet Arch compilé (makepkg), version de développement.
    Manual,
}

pub fn mode() -> Mode {
    if cfg!(debug_assertions) {
        return Mode::Manual;
    }
    mode_for(tauri::utils::platform::bundle_type())
}

fn mode_for(bundle: Option<BundleType>) -> Mode {
    match bundle {
        Some(BundleType::Nsis | BundleType::Msi | BundleType::App | BundleType::Dmg | BundleType::AppImage) => Mode::Auto,
        Some(BundleType::Deb | BundleType::Rpm) => Mode::OnDemand,
        None => Mode::Manual,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// "idle", "checking", "up_to_date", "available" (mode manuel), "downloading", "ready", "installing", "error"
    pub state: &'static str,
    pub mode: Mode,
    pub current: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub progress: Option<u8>,
    pub message: Option<String>,
    /// Date de la dernière vérification réussie (secondes Unix).
    pub checked_at: Option<u64>,
}

pub struct Updates {
    status: Mutex<Status>,
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    busy: AtomicBool,
    installing: AtomicBool,
}

impl Updates {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            status: Mutex::new(Status {
                state: "idle",
                mode: mode(),
                current: current_version(app),
                version: None,
                notes: None,
                progress: None,
                message: None,
                checked_at: None,
            }),
            ready: Mutex::new(None),
            busy: AtomicBool::new(false),
            installing: AtomicBool::new(false),
        }
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }
}

fn set(app: &AppHandle, f: impl FnOnce(&mut Status)) {
    let snapshot = {
        let updates = app.state::<Updates>();
        let mut status = updates.status.lock().unwrap();
        f(&mut status);
        status.clone()
    };
    let _ = app.emit_to("main", "update-status", snapshot);
}

/// Version de l'application (tauri.conf.json), celle que compare le plugin updater.
fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Vérifie s'il existe une nouvelle version et la télécharge (sauf en mode manuel).
pub async fn check(app: &AppHandle) -> Status {
    let updates = app.state::<Updates>();
    if updates.busy.swap(true, Ordering::SeqCst) {
        return updates.status();
    }
    if let Err(e) = check_inner(app).await {
        set(app, |s| {
            s.state = "error";
            s.progress = None;
            s.message = Some(e);
        });
    }
    updates.busy.store(false, Ordering::SeqCst);
    updates.status()
}

async fn check_inner(app: &AppHandle) -> Result<(), String> {
    if app.state::<Updates>().ready.lock().unwrap().is_some() {
        return Ok(()); // déjà téléchargée, en attente d'installation
    }
    set(app, |s| {
        s.state = "checking";
        s.message = None;
    });
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(60))
        // Windows : l'installateur ferme Coquille ; on retire l'icône pour ne pas laisser de fantôme.
        .on_before_exit(move || {
            let _ = handle.remove_tray_by_id("coquille");
        })
        .build()
        .map_err(|e| format!("mises à jour indisponibles : {e}"))?;
    let found = updater
        .check()
        .await
        .map_err(|e| format!("vérification des mises à jour impossible : {e}"))?;
    let now = unix_now();
    let Some(update) = found else {
        set(app, |s| {
            s.state = "up_to_date";
            s.version = None;
            s.notes = None;
            s.checked_at = Some(now);
        });
        return Ok(());
    };

    let version = update.version.clone();
    let notes = update.body.clone().filter(|n| !n.trim().is_empty());
    let manual = mode() == Mode::Manual;
    set(app, |s| {
        s.state = if manual { "available" } else { "downloading" };
        s.version = Some(version.clone());
        s.notes = notes;
        s.progress = if manual { None } else { Some(0) };
        s.checked_at = Some(now);
    });
    if manual {
        return Ok(());
    }

    let progress_app = app.clone();
    let mut received: u64 = 0;
    let mut shown: u8 = 0;
    let bytes = update
        .download(
            move |chunk, total| {
                received += chunk as u64;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let pct = (received * 100 / total).min(100) as u8;
                    if pct >= shown.saturating_add(5) {
                        shown = pct;
                        set(&progress_app, |s| s.progress = Some(pct));
                    }
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("téléchargement de la version {version} impossible : {e}"))?;
    *app.state::<Updates>().ready.lock().unwrap() = Some((update, bytes));
    set(app, |s| {
        s.state = "ready";
        s.progress = None;
    });
    Ok(())
}

/// Installe la version téléchargée puis relance Coquille. Ne revient pas en cas de succès.
/// `hidden` : Coquille redémarre sans afficher sa fenêtre.
pub fn install(app: &AppHandle, hidden: bool) -> Result<(), String> {
    let updates = app.state::<Updates>();
    let Some((update, bytes)) = updates.ready.lock().unwrap().take() else {
        return Err(match updates.status().state {
            "downloading" | "checking" => "La mise à jour est encore en cours de téléchargement.".into(),
            _ => "Aucune mise à jour téléchargée.".into(),
        });
    };
    if updates.installing.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    set(app, |s| s.state = "installing");
    write_marker(app, hidden);
    // Windows : l'installateur prend le relais et quitte Coquille (il la relance à la fin).
    match update.install(&bytes) {
        Ok(()) => app.restart(),
        Err(e) => {
            remove_marker(app);
            *updates.ready.lock().unwrap() = Some((update, bytes));
            updates.installing.store(false, Ordering::SeqCst);
            let message = format!("installation impossible : {e}");
            set(app, |s| {
                s.state = "error";
                s.message = Some(message.clone());
            });
            Err(message)
        }
    }
}

// ---------------------------------------------------------------------------
// Boucle d'arrière-plan
// ---------------------------------------------------------------------------

fn auto_install_enabled(app: &AppHandle) -> bool {
    app.state::<AppState>().config.lock().unwrap().auto_update
}

/// Moment sans risque de déranger : fenêtre cachée, pas de capture en cours, utilisateur absent (Windows).
fn quiet_moment(app: &AppHandle) -> bool {
    let window_visible = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if window_visible || app.state::<AppState>().capturing.load(Ordering::SeqCst) {
        return false;
    }
    match idle_time() {
        Some(idle) => idle >= IDLE_BEFORE_INSTALL,
        // macOS et AppImage : le redémarrage est quasi instantané et invisible fenêtre fermée.
        None => true,
    }
}

#[cfg(windows)]
fn idle_time() -> Option<Duration> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY : structure initialisée avec sa taille, comme l'exige l'API.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    let now = unsafe { GetTickCount() };
    Some(Duration::from_millis(now.wrapping_sub(info.dwTime) as u64))
}

#[cfg(not(windows))]
fn idle_time() -> Option<Duration> {
    None
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK).await;
        let mut next_check = Instant::now();
        // Après un échec d'installation automatique, on attend la vérification suivante pour réessayer.
        let mut install_failed = false;
        loop {
            if Instant::now() >= next_check {
                check(&app).await;
                next_check = Instant::now() + CHECK_EVERY;
                install_failed = false;
            }
            let ready = app.state::<Updates>().ready.lock().unwrap().is_some();
            if ready && !install_failed && mode() == Mode::Auto && auto_install_enabled(&app) && quiet_moment(&app) {
                install_failed = install(&app, true).is_err();
            }
            tokio::time::sleep(POLL).await;
        }
    });
}

// ---------------------------------------------------------------------------
// Repère laissé avant l'installation : au redémarrage, Coquille sait qu'elle vient d'être mise à jour.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Marker {
    from: String,
    hidden: bool,
}

fn marker_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(MARKER))
}

fn write_marker(app: &AppHandle, hidden: bool) {
    if let Some(path) = marker_path(app) {
        let marker = Marker { from: current_version(app), hidden };
        if let Ok(json) = serde_json::to_string(&marker) {
            let _ = std::fs::create_dir_all(path.parent().unwrap_or(&path));
            let _ = std::fs::write(path, json);
        }
    }
}

fn remove_marker(app: &AppHandle) {
    if let Some(path) = marker_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

/// Au démarrage : (redémarrer caché ?, message à afficher). Le repère est supprimé.
pub fn take_marker(app: &AppHandle) -> (bool, Option<String>) {
    let Some(path) = marker_path(app) else { return (false, None) };
    let Ok(text) = std::fs::read_to_string(&path) else { return (false, None) };
    let _ = std::fs::remove_file(&path);
    let Ok(marker) = serde_json::from_str::<Marker>(&text) else { return (false, None) };
    (marker.hidden, after_update_notice(&marker.from, &current_version(app)))
}

fn after_update_notice(from: &str, now: &str) -> Option<String> {
    if from == now {
        Some(format!(
            "La mise à jour de Coquille n'a pas abouti : la version {now} est toujours installée. \
             Elle sera retentée ; sinon, installe la dernière version depuis GitHub."
        ))
    } else {
        Some(format!("Coquille a été mise à jour : version {from} → {now}."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_by_bundle() {
        assert_eq!(mode_for(Some(BundleType::Nsis)), Mode::Auto);
        assert_eq!(mode_for(Some(BundleType::Msi)), Mode::Auto);
        assert_eq!(mode_for(Some(BundleType::App)), Mode::Auto);
        assert_eq!(mode_for(Some(BundleType::AppImage)), Mode::Auto);
        assert_eq!(mode_for(Some(BundleType::Deb)), Mode::OnDemand);
        assert_eq!(mode_for(Some(BundleType::Rpm)), Mode::OnDemand);
        // Paquet Arch compilé avec cargo : le binaire n'est pas marqué par le bundler de Tauri.
        assert_eq!(mode_for(None), Mode::Manual);
    }

    #[test]
    fn notices() {
        assert_eq!(after_update_notice("0.4.0", "0.4.1").unwrap(), "Coquille a été mise à jour : version 0.4.0 → 0.4.1.");
        assert!(after_update_notice("0.4.1", "0.4.1").unwrap().contains("n'a pas abouti"));
    }
}
