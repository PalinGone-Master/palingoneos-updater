// SPDX-License-Identifier: GPL-3.0-or-later

use crate::config::Config;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::wayland::commands::popup::{
    destroy_popup, get_popup,
};
use cosmic::iced::{futures, window::Id, Limits, Subscription};
use cosmic::prelude::*;
use cosmic::widget;
use futures::SinkExt;
use serde::Deserialize;

const PALINGONE_LOGO: &[u8] = include_bytes!("../resources/icon.svg");

#[derive(Deserialize)]
struct Release {
    tag_name: String,
}

#[derive(Debug, Clone)]
enum UpdateStatus {
    Checking,
    UpToDate(String),
    UpdateAvailable(String),
    Updating(String),
    UpdateSucceeded(String),
    Error(String),
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self::Checking
    }
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    update_status: UpdateStatus,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    SubscriptionChannel,
    UpdateConfig(Config),
    UpdateCheckFinished(UpdateStatus),
    StartUpdate(String),
    UpdateFinished(UpdateStatus),
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.palingoneos.updater";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let app = AppModel {
            core,
            config: cosmic_config::Config::new(Self::APP_ID, Config::VERSION)
                .map(|context| match Config::get_entry(&context) {
                    Ok(config) => config,
                    Err((_errors, config)) => config,
                })
                .unwrap_or_default(),
            update_status: UpdateStatus::Checking,
            ..Default::default()
        };

        let task = Task::perform(check_for_updates(), Message::UpdateCheckFinished)
            .map(cosmic::Action::App);

        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icon = cosmic::widget::icon::from_svg_bytes(PALINGONE_LOGO);

        self.core
            .applet
            .icon_button_from_handle(icon)
            .on_press(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let mut content = widget::column(vec![
            widget::text::title3("PalinGoneOS Update").into(),
        ]);

        match &self.update_status {
            UpdateStatus::Checking => {
                content = content.push(
                    widget::text("Vérification des mises à jour…"),
                );
            }

            UpdateStatus::UpToDate(version) => {
                content = content.push(
                    widget::text(format!(
                        "✓ PalinGoneOS est à jour\nVersion {}",
                        version
                    )),
                );
            }

            UpdateStatus::UpdateAvailable(version) => {
                content = content
                    .push(
                        widget::text(format!(
                            "↑ Mise à jour disponible\nVersion {}",
                            version
                        )),
                    )
                    .push(
                        widget::button::suggested("Mettre à jour")
                            .on_press(Message::StartUpdate(version.clone())),
                    );
            }

            UpdateStatus::Error(error) => {
                content = content.push(
                    widget::text(format!("Erreur :\n{}", error)),
                );
            }
       
            UpdateStatus::Updating(version) => {
                content = content.push(
                    widget::text(format!(
                        "Mise à jour de PalinGoneOS en cours…\nVersion {}",
                        version
                    )),
                );
            }

            UpdateStatus::UpdateSucceeded(version) => {
                content = content.push(
                    widget::text(format!(
                        "✓ PalinGoneOS a été mis à jour\nVersion {}",
                        version
                    )),
                );
            }
        }

        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        Subscription::batch(vec![
            Subscription::run(|| {
                cosmic::iced::stream::channel(
                    4,
                    move |mut channel: futures::channel::mpsc::Sender<_>| async move {
                        _ = channel.send(Message::SubscriptionChannel).await;
                        futures::future::pending().await
                    },
                )
            }),

            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ])
    }

    fn update(
        &mut self,
        message: Self::Message,
    ) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::SubscriptionChannel => {}

            Message::UpdateConfig(config) => {
                self.config = config;
            }

            Message::UpdateCheckFinished(status) => {
                self.update_status = status;
            }

            Message::UpdateFinished(status) => {
                self.update_status = status;
            }

            Message::StartUpdate(version) => {
                self.update_status = UpdateStatus::Updating(version.clone());

                return Task::perform(
                    run_update(version),
                    Message::UpdateFinished,
                )
                .map(cosmic::Action::App);
            }

            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);

                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );

                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(200.0)
                        .max_height(1080.0);

                    get_popup(popup_settings)
                };
            }

            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }

        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

async fn check_for_updates() -> UpdateStatus {
    // Version actuellement installée[cite: 1]
    let local_version = match std::fs::read_to_string("/etc/palingoneos/version") {
        Ok(version) => version.trim().to_string(),
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de lire la version de PalinGoneOS : {}",
                error
            ));
        }
    };

    let local_parts: Vec<&str> = local_version.split('.').collect();

    if local_parts.len() != 3 {
        return UpdateStatus::Error(format!(
            "Version locale invalide : {}",
            local_version
        ));
    }

    let Ok(local_major) = local_parts[0].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version locale invalide : {}", local_version));
    };
    let Ok(local_minor) = local_parts[1].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version locale invalide : {}", local_version));
    };
    let Ok(local_patch) = local_parts[2].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version locale invalide : {}", local_version));
    };

    // Interrogation de l'API GitHub Releases via ureq avec le bon dépôt
    let response = match ureq::get("https://api.github.com/repos/PalinGone-Master/PalinGoneOS/releases/latest")
        .set("User-Agent", "PalinGoneOS-Updater")
        .call() 
    {
        Ok(resp) => resp,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible d'interroger l'API GitHub : {}",
                error
            ));
        }
    };

    let release: Release = match response.into_json() {
        Ok(rel) => rel,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de parser la réponse JSON : {}",
                error
            ));
        }
    };

    let remote_version_raw = release.tag_name.trim_start_matches('v');
    let remote_parts: Vec<&str> = remote_version_raw.split('.').collect();

    if remote_parts.len() != 3 {
        return UpdateStatus::Error(format!(
            "Version distante invalide : {}",
            remote_version_raw
        ));
    }

    let Ok(remote_major) = remote_parts[0].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version distante invalide : {}", remote_version_raw));
    };
    let Ok(remote_minor) = remote_parts[1].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version distante invalide : {}", remote_version_raw));
    };
    let Ok(remote_patch) = remote_parts[2].parse::<u32>() else {
        return UpdateStatus::Error(format!("Version distante invalide : {}", remote_version_raw));
    };

    let remote_version = format!(
        "{}.{}.{}",
        remote_major, remote_minor, remote_patch
    );

    if (remote_major, remote_minor, remote_patch)
        > (local_major, local_minor, local_patch)
    {
        UpdateStatus::UpdateAvailable(remote_version)
    } else {
        UpdateStatus::UpToDate(local_version)
    }
}

async fn run_update(version: String) -> UpdateStatus {
    let repo = "/etc/nixos";
    let tag = format!("v{}", version);

    // Récupère les tags depuis le dépôt distant[cite: 1].
    let fetch = match std::process::Command::new("git")
        .args(["-C", repo, "fetch", "origin", "--tags"])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de récupérer les mises à jour : {}",
                error
            ));
        }
    };

    if !fetch.status.success() {
        let error = String::from_utf8_lossy(&fetch.stderr);
        return UpdateStatus::Error(format!(
            "Échec de la récupération Git : {}",
            error.trim()
        ));
    }

    // Vérifie que la version demandée existe bien[cite: 1].
    let tag_check = match std::process::Command::new("git")
        .args([
            "-C",
            repo,
            "rev-parse",
            &format!("refs/tags/{}", tag),
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de vérifier {} : {}",
                version, error
            ));
        }
    };

    if !tag_check.status.success() {
        return UpdateStatus::Error(format!(
            "La version {} n'existe pas dans le dépôt.",
            version
        ));
    }

    // Exécution avec capture des erreurs au lieu de les ignorer[cite: 1]
    let output = match std::process::Command::new("pkexec")
        .env("SHELL", "/run/current-system/sw/bin/bash")
        .args([
            "/run/current-system/sw/bin/palingoneos-update-helper",
            &version,
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de lancer la mise à jour : {}",
                error
            ));
        }
    };

    if output.status.success() {
        UpdateStatus::UpdateSucceeded(version)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            "Erreur inconnue.".to_string()
        };
        UpdateStatus::Error(format!("Échec :\n{}", details))
    }
}
