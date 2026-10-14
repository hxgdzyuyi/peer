use super::*;

pub(super) struct SettingsForm {
    pub draft: Preferences,
    tab: &'static str,
    git: Entity<InputState>,
    filter: Entity<InputState>,
    executables: Vec<Entity<InputState>>,
    python: Entity<InputState>,
    advanced: bool,
}
impl SettingsForm {
    fn new(preferences: &Preferences, window: &mut Window, cx: &mut App) -> Self {
        let mut draft = preferences.clone();
        for mut preset in peer_core::navigation::default_servers() {
            if !draft.language_servers.iter().any(|s| {
                s.id == preset.id
                    || s.languages
                        .keys()
                        .any(|ext| preset.languages.keys().any(|p| p.eq_ignore_ascii_case(ext)))
            }) {
                preset.enabled = false;
                draft.language_servers.push(preset);
            }
        }
        let git = input("Auto-detect Git", window, cx);
        git.update(cx, |s, cx| {
            s.set_value(draft.git_executable.clone().unwrap_or_default(), window, cx)
        });
        let python = input("Default Python environment", window, cx);
        let interpreter = draft
            .language_servers
            .iter()
            .find(|s| s.id == "python")
            .and_then(|s| s.settings["python"]["pythonPath"].as_str())
            .unwrap_or("");
        python.update(cx, |s, cx| s.set_value(interpreter, window, cx));
        let executables = draft
            .language_servers
            .iter()
            .map(|server| {
                let field = input("Language server executable", window, cx);
                field.update(cx, |s, cx| {
                    s.set_value(server.executable.clone(), window, cx)
                });
                field
            })
            .collect();
        Self {
            draft,
            tab: "Appearance",
            git,
            filter: input("Search themes…", window, cx),
            executables,
            python,
            advanced: false,
        }
    }
    pub(super) fn value(
        &self,
        advanced: &Entity<TextareaState>,
        cx: &App,
    ) -> Result<Preferences, String> {
        let mut p = self.draft.clone();
        let git = self.git.read(cx).value().trim().to_owned();
        p.git_executable = (!git.is_empty()).then_some(git);
        if self.advanced {
            p.language_servers = serde_json::from_str(&advanced.read(cx).value())
                .map_err(|e| format!("Language server configuration: {e}"))?;
        } else {
            for (server, input) in p.language_servers.iter_mut().zip(&self.executables) {
                server.executable = input.read(cx).value().trim().into();
                if server.id == "python" {
                    let path = self.python.read(cx).value().trim().to_string();
                    if !server.settings.is_object() {
                        server.settings = serde_json::json!({});
                    }
                    if !server.settings["python"].is_object() {
                        server.settings["python"] = serde_json::json!({});
                    }
                    let python = server.settings["python"].as_object_mut().unwrap();
                    if path.is_empty() {
                        python.remove("pythonPath");
                    } else {
                        python.insert("pythonPath".into(), path.into());
                    }
                }
            }
        }
        Ok(p)
    }
}
impl Workspace {
    pub(super) fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let form = SettingsForm::new(&self.preferences, window, cx);
        self.subscriptions
            .push(cx.subscribe(&form.filter, |_, _, _: &InputEvent, cx| cx.notify()));
        self.settings_form = Some(form);
        self.settings_visible = true;
        cx.notify();
    }
    pub(super) fn settings_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let form = self.settings_form.as_ref().expect("settings form");
        let mut tabs = h_flex().gap_2();
        for label in ["Appearance", "Viewer", "Code navigation", "Git"] {
            tabs = tabs.child(
                Button::new(label)
                    .label(label)
                    .selected(form.tab == label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(f) = &mut this.settings_form {
                            f.tab = label;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut body = v_flex().gap_3().p_3();
        match form.tab {
            "Appearance" => {
                let mut modes = h_flex().gap_2();
                for mode in ["system", "light", "dark"] {
                    modes = modes.child(
                        Button::new(mode)
                            .label(mode)
                            .selected(form.draft.appearance.mode == mode)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let f = this.settings_form.as_mut().unwrap();
                                f.draft.appearance.mode = mode.into();
                                let dark = mode == "dark"
                                    || (mode == "system"
                                        && matches!(
                                            window.appearance(),
                                            WindowAppearance::Dark | WindowAppearance::VibrantDark
                                        ));
                                Theme::change(
                                    if dark {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    },
                                    Some(window),
                                    cx,
                                );
                                let id = if dark {
                                    &f.draft.appearance.dark_theme
                                } else {
                                    &f.draft.appearance.light_theme
                                };
                                if let Some(t) = this.themes.iter().find(|t| &t.id == id) {
                                    crate::themes::apply(t, f.draft.code_font_size, cx);
                                }
                                cx.notify();
                            })),
                    );
                }
                body = body.child(modes).child(Input::new(&form.filter)).child(
                    Button::new("import-theme")
                        .label("Import theme…")
                        .on_click(cx.listener(|this, _, w, c| this.import_theme(w, c))),
                );
                let query = form.filter.read(cx).value().to_lowercase();
                for kind in ["light", "dark"] {
                    let mut choices = h_flex().gap_2().flex_wrap();
                    for theme in self
                        .themes
                        .iter()
                        .filter(|t| t.kind == kind && t.name.to_lowercase().contains(&query))
                    {
                        let id = theme.id.clone();
                        let remove = id.clone();
                        let theme_copy = theme.clone();
                        let selected = if kind == "light" {
                            &form.draft.appearance.light_theme
                        } else {
                            &form.draft.appearance.dark_theme
                        } == &id;
                        let bg = theme.definition["colors"]["editor.background"]
                            .as_str()
                            .and_then(|v| Hsla::parse_hex(v).ok())
                            .unwrap_or_else(|| {
                                if kind == "light" {
                                    rgb(0xffffff).into()
                                } else {
                                    rgb(0x181818).into()
                                }
                            });
                        let fg = theme.definition["colors"]["editor.foreground"]
                            .as_str()
                            .and_then(|v| Hsla::parse_hex(v).ok())
                            .unwrap_or_else(|| {
                                if kind == "light" {
                                    rgb(0x24292e).into()
                                } else {
                                    rgb(0xe6edf3).into()
                                }
                            });
                        let mut card = v_flex()
                            .w(px(235.))
                            .gap_1()
                            .child(
                                div()
                                    .p_2()
                                    .bg(bg)
                                    .text_color(fg)
                                    .child("fn review() → Diff\n  − old   + new"),
                            )
                            .child(
                                Button::new(SharedString::from(format!("theme-{id}")))
                                    .label(theme.name.clone())
                                    .selected(selected)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let f = this.settings_form.as_mut().unwrap();
                                        if kind == "light" {
                                            f.draft.appearance.light_theme = id.clone();
                                        } else {
                                            f.draft.appearance.dark_theme = id.clone();
                                        }
                                        f.draft.appearance.mode = kind.into();
                                        Theme::change(
                                            if kind == "light" {
                                                ThemeMode::Light
                                            } else {
                                                ThemeMode::Dark
                                            },
                                            Some(window),
                                            cx,
                                        );
                                        crate::themes::apply(
                                            &theme_copy,
                                            f.draft.code_font_size,
                                            cx,
                                        );
                                        cx.notify();
                                    })),
                            );
                        if !crate::themes::builtins().iter().any(|t| t.id == remove) {
                            card = card.child(
                                Button::new(SharedString::from(format!("remove-{remove}")))
                                    .small()
                                    .label("Remove theme")
                                    .on_click(cx.listener(move |this, _, w, c| {
                                        this.remove_theme(remove.clone(), w, c)
                                    })),
                            );
                        }
                        choices = choices.child(card);
                    }
                    body = body.child(format!("{kind} theme")).child(choices);
                }
            }
            "Viewer" => {
                for (name, value, min, max) in [
                    ("Font size", form.draft.code_font_size, 10, 32),
                    ("Tab width", form.draft.tab_width, 1, 16),
                ] {
                    let mut row = h_flex()
                        .gap_2()
                        .child(div().w(px(130.)).child(format!("{name}: {value}")));
                    for (label, delta) in [("−", -1), ("+", 1)] {
                        row = row.child(
                            Button::new(SharedString::from(format!("{name}-{label}")))
                                .label(label)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let f = this.settings_form.as_mut().unwrap();
                                    let v = if name == "Font size" {
                                        &mut f.draft.code_font_size
                                    } else {
                                        &mut f.draft.tab_width
                                    };
                                    *v = (*v as i32 + delta).clamp(min, max) as u32;
                                    cx.notify();
                                })),
                        );
                    }
                    body = body.child(row);
                }
                body = body.child(
                    Button::new("whitespace")
                        .label("Show whitespace")
                        .selected(form.draft.show_whitespace)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let f = this.settings_form.as_mut().unwrap();
                            f.draft.show_whitespace = !f.draft.show_whitespace;
                            cx.notify();
                        })),
                );
                let mut layouts = h_flex().gap_2();
                for layout in ["unified", "split"] {
                    layouts = layouts.child(
                        Button::new(layout)
                            .label(layout)
                            .selected(form.draft.diff_layout == layout)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings_form.as_mut().unwrap().draft.diff_layout =
                                    layout.into();
                                cx.notify();
                            })),
                    );
                }
                body = body
                    .child("Diff layout")
                    .child(layouts)
                    .child("Tab width is the fallback; repository EditorConfig takes precedence.");
            }
            "Git" => {
                body=body.child("Git executable path").child(Input::new(&form.git)).child("Leave empty to use installed Git. Applied on the next repository open or refresh.");
            }
            _ => {
                body = body.child(
                    "F12: definitions · Shift+F12: references. Install language servers locally.",
                );
                if form.advanced {
                    body = body
                        .child("Advanced / custom servers (JSON)")
                        .child(Textarea::new(&self.settings).h(px(360.)));
                } else {
                    for (index, server) in form.draft.language_servers.iter().enumerate() {
                        body = body.child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new(("server", index))
                                        .label(format!(
                                            "{} · {}",
                                            if server.enabled {
                                                "Enabled"
                                            } else {
                                                "Disabled"
                                            },
                                            server.name
                                        ))
                                        .selected(server.enabled)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let s = &mut this
                                                .settings_form
                                                .as_mut()
                                                .unwrap()
                                                .draft
                                                .language_servers[index];
                                            s.enabled = !s.enabled;
                                            cx.notify();
                                        })),
                                )
                                .child(Input::new(&form.executables[index]).flex_1()),
                        );
                        if server.id == "python" {
                            body = body
                                .child("Python interpreter (optional)")
                                .child(Input::new(&form.python));
                        }
                    }
                }
                body = body.child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("advanced-servers")
                                .label(if form.advanced {
                                    "Cancel JSON changes"
                                } else {
                                    "Advanced / custom servers"
                                })
                                .on_click(cx.listener(|this, _, w, c| {
                                    let f = this.settings_form.as_mut().unwrap();
                                    if !f.advanced {
                                        if let Ok(p) = f.value(&this.settings, c) {
                                            this.settings.update(c, |s, c| {
                                                s.set_value(
                                                    serde_json::to_string_pretty(
                                                        &p.language_servers,
                                                    )
                                                    .unwrap(),
                                                    w,
                                                    c,
                                                )
                                            });
                                        }
                                    }
                                    f.advanced = !f.advanced;
                                    c.notify();
                                })),
                        )
                        .child(
                            Button::new("restore-servers")
                                .label("Restore presets")
                                .on_click(cx.listener(|this, _, w, c| {
                                    let mut p = this
                                        .settings_form
                                        .as_ref()
                                        .unwrap()
                                        .value(&this.settings, c)
                                        .unwrap_or_else(|_| this.preferences.clone());
                                    p.language_servers = peer_core::navigation::default_servers();
                                    let mut f = SettingsForm::new(&p, w, c);
                                    f.tab = "Code navigation";
                                    this.settings_form = Some(f);
                                    c.notify();
                                })),
                        ),
                );
            }
        }
        let _ = window;
        v_flex()
            .size_full()
            .gap_2()
            .child(tabs)
            .child(
                div()
                    .id("settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .child(
                h_flex()
                    .p_2()
                    .gap_2()
                    .child(
                        Button::new("save-settings")
                            .primary()
                            .label("Save settings")
                            .on_click(cx.listener(|this, _, w, c| this.save_settings(w, c))),
                    )
                    .child(
                        Button::new("close-settings")
                            .label("Cancel / close")
                            .on_click(cx.listener(|this, _, w, c| {
                                this.settings_visible = false;
                                this.apply_theme(w, c);
                                c.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
