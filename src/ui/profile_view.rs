use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, Button, Image, Label, MenuButton, Orientation, Popover};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, AlertDialog, Clamp, PreferencesGroup, ResponseAppearance};

use crate::config::{AppConfig, ProfileConfig};

#[derive(Debug, Clone, PartialEq)]
pub enum ProfileAction {
    Activate(String),
    Create { name: String },
    Duplicate(String),
    Rename { old_name: String, new_name: String },
    Delete(String),
}

pub type ProfileActionCallback = Rc<dyn Fn(ProfileAction)>;

pub fn format_profile_summary(profile: &ProfileConfig) -> String {
    let map_count = profile.key_bindings.len();
    let map_str = if map_count == 1 {
        "1 Mapping".to_string()
    } else {
        format!("{} Mappings", map_count)
    };
    format!(
        "{} • {} Hz • {} Curve • {}",
        profile.controller_type, profile.polling_rate_hz, profile.curve_preset, map_str
    )
}

pub struct ProfileView {
    pub container: GtkBox,
    content_box: GtkBox,
    on_action: Option<ProfileActionCallback>,
}

impl ProfileView {
    pub fn new(config: &AppConfig, on_action: Option<ProfileActionCallback>) -> Self {
        let outer_container = GtkBox::new(Orientation::Vertical, 16);
        outer_container.set_margin_start(16);
        outer_container.set_margin_end(16);
        outer_container.set_margin_top(16);
        outer_container.set_margin_bottom(24);
        outer_container.set_hexpand(true);

        let content_box = GtkBox::new(Orientation::Vertical, 16);
        content_box.set_hexpand(true);

        let clamp = Clamp::new();
        clamp.set_maximum_size(860);
        clamp.set_child(Some(&content_box));
        outer_container.append(&clamp);

        let view = Self {
            container: outer_container,
            content_box,
            on_action,
        };

        view.refresh(config);
        view
    }

    pub fn refresh(&self, config: &AppConfig) {
        while let Some(child) = self.content_box.first_child() {
            self.content_box.remove(&child);
        }

        let group = PreferencesGroup::new();
        group.set_title("Gamepad Profiles");
        group.set_description(Some(
            "Select an active configuration or manage custom mapping profiles.",
        ));

        // new profile button
        let new_profile_btn = Button::with_label("New Profile…");
        new_profile_btn.add_css_class("suggested-action");
        new_profile_btn.set_valign(Align::Center);

        let on_act_create = self.on_action.clone();
        let new_btn_clone = new_profile_btn.clone();
        new_profile_btn.connect_clicked(move |_| {
            let dialog = AlertDialog::new(
                Some("Create New Profile"),
                Some("Enter a name for the new gamepad profile:"),
            );
            let entry = gtk4::Entry::new();
            entry.set_text("New Profile");
            entry.set_margin_start(12);
            entry.set_margin_end(12);
            entry.select_region(0, -1);
            entry.connect_map(|e| {
                e.grab_focus();
                e.select_region(0, -1);
            });
            dialog.set_extra_child(Some(&entry));
            dialog.set_focus(Some(&entry));
            entry.set_activates_default(true);
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("create", "Create");
            dialog.set_response_appearance("create", ResponseAppearance::Suggested);
            dialog.set_default_response(Some("create"));
            dialog.set_close_response("cancel");

            let d_clone = dialog.clone();
            entry.connect_activate(move |_| {
                if let Some(default_widget) = d_clone.default_widget()
                    && let Ok(btn) = default_widget.downcast::<gtk4::Button>()
                {
                    btn.emit_clicked();
                } else {
                    d_clone.emit_by_name::<()>("response", &[&"create"]);
                }
            });

            let on_a = on_act_create.clone();
            dialog.choose(
                &new_btn_clone,
                None::<&gtk4::gio::Cancellable>,
                move |response| {
                    if response == "create" {
                        let name_text = entry.text().to_string();
                        let trimmed = if name_text.trim().is_empty() {
                            "New Profile".to_string()
                        } else {
                            name_text.trim().to_string()
                        };
                        if let Some(ref cb) = on_a {
                            cb(ProfileAction::Create { name: trimmed });
                        }
                    }
                },
            );
        });
        group.set_header_suffix(Some(&new_profile_btn));

        for profile in &config.profiles {
            let is_active = profile.name == config.active_profile;
            let row = ActionRow::new();
            row.set_title(&profile.name);
            row.set_subtitle(&format_profile_summary(profile));
            row.set_activatable(true);

            // status icon
            let icon_name = if is_active {
                "object-select-symbolic"
            } else {
                "input-gaming-symbolic"
            };
            let icon = Image::from_icon_name(icon_name);
            if is_active {
                icon.add_css_class("accent");
            } else {
                icon.add_css_class("dimmed");
            }
            row.add_prefix(&icon);

            // click row to activate profile
            if !is_active {
                let name_clone = profile.name.clone();
                let on_act = self.on_action.clone();
                row.connect_activated(move |_| {
                    if let Some(ref cb) = on_act {
                        cb(ProfileAction::Activate(name_clone.clone()));
                    }
                });
            }

            // active badge
            if is_active {
                let active_badge = Label::new(Some("Active"));
                active_badge.add_css_class("pill");
                active_badge.add_css_class("heading");
                active_badge.set_valign(Align::Center);
                row.add_suffix(&active_badge);
            }

            // overflow actions menu
            let menu_btn = MenuButton::new();
            menu_btn.set_icon_name("view-more-symbolic");
            menu_btn.set_tooltip_text(Some("Profile Options"));
            menu_btn.add_css_class("flat");
            menu_btn.set_valign(Align::Center);

            let popover = Popover::new();
            let pop_box = GtkBox::new(Orientation::Vertical, 4);
            pop_box.set_margin_start(6);
            pop_box.set_margin_end(6);
            pop_box.set_margin_top(6);
            pop_box.set_margin_bottom(6);

            // duplicate profile action
            let dup_btn = Button::builder()
                .icon_name("edit-copy-symbolic")
                .label("Duplicate")
                .has_frame(false)
                .build();
            dup_btn.add_css_class("flat");
            dup_btn.set_halign(Align::Fill);

            let pop_clone_dup = popover.clone();
            let name_dup = profile.name.clone();
            let on_act_dup = self.on_action.clone();
            dup_btn.connect_clicked(move |_| {
                pop_clone_dup.popdown();
                if let Some(ref cb) = on_act_dup {
                    cb(ProfileAction::Duplicate(name_dup.clone()));
                }
            });
            pop_box.append(&dup_btn);

            // rename profile action
            let rename_btn = Button::builder()
                .icon_name("document-edit-symbolic")
                .label("Rename…")
                .has_frame(false)
                .build();
            rename_btn.add_css_class("flat");
            rename_btn.set_halign(Align::Fill);

            let pop_clone_ren = popover.clone();
            let old_name_ren = profile.name.clone();
            let on_act_ren = self.on_action.clone();
            let menu_btn_ren = menu_btn.clone();
            rename_btn.connect_clicked(move |_| {
                pop_clone_ren.popdown();
                let dialog = AlertDialog::new(
                    Some("Rename Profile"),
                    Some("Enter a new name for this profile:"),
                );
                let entry = gtk4::Entry::new();
                entry.set_text(&old_name_ren);
                entry.set_margin_start(12);
                entry.set_margin_end(12);
                entry.select_region(0, -1);
                entry.connect_map(|e| {
                    e.grab_focus();
                    e.select_region(0, -1);
                });
                dialog.set_extra_child(Some(&entry));
                dialog.set_focus(Some(&entry));
                entry.set_activates_default(true);
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("rename", "Rename");
                dialog.set_response_appearance("rename", ResponseAppearance::Suggested);
                dialog.set_default_response(Some("rename"));
                dialog.set_close_response("cancel");

                let d_clone = dialog.clone();
                entry.connect_activate(move |_| {
                    if let Some(default_widget) = d_clone.default_widget()
                        && let Ok(btn) = default_widget.downcast::<gtk4::Button>()
                    {
                        btn.emit_clicked();
                    } else {
                        d_clone.emit_by_name::<()>("response", &[&"rename"]);
                    }
                });

                let old_n = old_name_ren.clone();
                let on_a = on_act_ren.clone();
                dialog.choose(
                    &menu_btn_ren,
                    None::<&gtk4::gio::Cancellable>,
                    move |response| {
                        if response == "rename" {
                            let trimmed = entry.text().to_string().trim().to_string();
                            if !trimmed.is_empty()
                                && trimmed != old_n
                                && let Some(ref cb) = on_a
                            {
                                cb(ProfileAction::Rename {
                                    old_name: old_n,
                                    new_name: trimmed,
                                });
                            }
                        }
                    },
                );
            });
            pop_box.append(&rename_btn);

            // delete profile action
            let del_btn = Button::builder()
                .icon_name("user-trash-symbolic")
                .label("Delete")
                .has_frame(false)
                .build();
            del_btn.add_css_class("flat");
            del_btn.add_css_class("destructive-action");
            del_btn.set_halign(Align::Fill);
            del_btn.set_sensitive(config.profiles.len() > 1);

            let pop_clone_del = popover.clone();
            let name_del = profile.name.clone();
            let on_act_del = self.on_action.clone();
            let menu_btn_del = menu_btn.clone();
            del_btn.connect_clicked(move |_| {
                pop_clone_del.popdown();
                let dialog = AlertDialog::new(
                    Some(&format!("Delete \"{}\"?", name_del)),
                    Some("Are you sure you want to delete this profile? This action cannot be undone."),
                );
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("delete", "Delete");
                dialog.set_response_appearance("delete", ResponseAppearance::Destructive);
                dialog.set_default_response(Some("cancel"));
                dialog.set_close_response("cancel");

                let n_del = name_del.clone();
                let on_a = on_act_del.clone();
                dialog.choose(
                    &menu_btn_del,
                    None::<&gtk4::gio::Cancellable>,
                    move |response| {
                        if response == "delete"
                            && let Some(ref cb) = on_a
                        {
                            cb(ProfileAction::Delete(n_del));
                        }
                    },
                );
            });
            pop_box.append(&del_btn);

            popover.set_child(Some(&pop_box));
            menu_btn.set_popover(Some(&popover));
            row.add_suffix(&menu_btn);

            group.add(&row);
        }

        self.content_box.append(&group);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProfileConfig;

    #[test]
    fn test_format_profile_summary() {
        let prof = ProfileConfig::default_profile();
        let summary = format_profile_summary(&prof);
        assert!(summary.contains("Xbox Controller"));
        assert!(summary.contains("1000 Hz"));
        assert!(summary.contains("Linear Curve"));
        assert!(summary.contains("8 Mappings"));

        let fps = ProfileConfig::fps_profile();
        let fps_summary = format_profile_summary(&fps);
        assert!(fps_summary.contains("Aggro Curve"));
        assert!(fps_summary.contains("10 Mappings"));

        let racing = ProfileConfig::racing_profile();
        let racing_summary = format_profile_summary(&racing);
        assert!(racing_summary.contains("Smooth Curve"));
        assert!(racing_summary.contains("7 Mappings"));

        let mut custom = ProfileConfig::default_profile();
        custom.key_bindings.clear();
        custom
            .key_bindings
            .insert(crate::keyboard::KeyId::W, "LS-U".to_string());
        let single_summary = format_profile_summary(&custom);
        assert!(single_summary.contains("1 Mapping"));
    }

    #[test]
    fn test_profile_action_variants() {
        let act1 = ProfileAction::Activate("FPS Tactical".to_string());
        let act2 = ProfileAction::Create {
            name: "Custom".to_string(),
        };
        let act3 = ProfileAction::Duplicate("Default".to_string());
        let act4 = ProfileAction::Rename {
            old_name: "A".to_string(),
            new_name: "B".to_string(),
        };
        let act5 = ProfileAction::Delete("FPS Tactical".to_string());

        assert_eq!(act1, ProfileAction::Activate("FPS Tactical".to_string()));
        assert_ne!(act1, act3);
        assert_eq!(
            act2,
            ProfileAction::Create {
                name: "Custom".to_string(),
            }
        );
        assert_eq!(
            act4,
            ProfileAction::Rename {
                old_name: "A".to_string(),
                new_name: "B".to_string(),
            }
        );
        assert_eq!(act5, ProfileAction::Delete("FPS Tactical".to_string()));
    }
}
