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
use std::process::Command;

const PALINGONE_LOGO: &[u8] = include_bytes!("../resources/icon.svg");

#[derive(Debug, Clone)]
enum UpdateStatus {
    Checking,
    UpToDate(String),
    UpdateAvailable(String),
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
    }

    self.core.applet.popup_container(content).into()
}

    fn subscription(&self) -> Subscription<Self::Message> {
        struct MySubscription;

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
            Message::StartUpdate(version) => {
                println!("Mise à jour demandée : {}", version);

                let _ = Command::new("pkexec")
                    .args([
                        "/run/current-system/sw/bin/palingoneos-update-helper",
                    ])
                    .status();
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
    // Version actuellement installée
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

    let local_major = match local_parts[0].parse::<u32>() {
        Ok(value) => value,
        Err(_) => {
            return UpdateStatus::Error(format!(
                "Version locale invalide : {}",
                local_version
            ));
        }
    };

    let local_minor = match local_parts[1].parse::<u32>() {
        Ok(value) => value,
        Err(_) => {
            return UpdateStatus::Error(format!(
                "Version locale invalide : {}",
                local_version
            ));
        }
    };

    let local_patch = match local_parts[2].parse::<u32>() {
        Ok(value) => value,
        Err(_) => {
            return UpdateStatus::Error(format!(
                "Version locale invalide : {}",
                local_version
            ));
        }
    };

    // Récupération des tags du dépôt PalinGoneOS
    let remote_output = match Command::new("git")
        .args([
            "-C",
            "/etc/nixos",
            "ls-remote",
            "--tags",
            "origin",
        ])
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            return UpdateStatus::Error(format!(
                "Impossible de lancer git : {}",
                error
            ));
        }
    };

    if !remote_output.status.success() {
        let error = String::from_utf8_lossy(&remote_output.stderr);

        return UpdateStatus::Error(format!(
            "Impossible d'interroger le dépôt : {}",
            error.trim()
        ));
    }

    let remote_tags = String::from_utf8_lossy(&remote_output.stdout);

    let mut highest_version: Option<(u32, u32, u32)> = None;

    for line in remote_tags.lines() {
        let Some(reference) = line.split_whitespace().nth(1) else {
            continue;
        };

        let Some(version) = reference.strip_prefix("refs/tags/v") else {
            continue;
        };

        let parts: Vec<&str> = version.split('.').collect();

        if parts.len() != 3 {
            continue;
        }

        let Ok(major) = parts[0].parse::<u32>() else {
            continue;
        };

        let Ok(minor) = parts[1].parse::<u32>() else {
            continue;
        };

        let Ok(patch) = parts[2].parse::<u32>() else {
            continue;
        };

        let candidate = (major, minor, patch);

        if highest_version
            .as_ref()
            .is_none_or(|current| candidate > *current)
        {
            highest_version = Some(candidate);
        }
    }

    let Some((remote_major, remote_minor, remote_patch)) = highest_version else {
        return UpdateStatus::Error(
            "Aucun tag vX.Y.Z trouvé dans le dépôt.".to_string(),
        );
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

fn extract_json_value(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", key);
    let start = json.find(&pattern)? + pattern.len();
    let remaining = &json[start..];
    let end = remaining.find('"')?;

    Some(remaining[..end].to_string())
}
