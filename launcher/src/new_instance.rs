//! New instance's kinds (docs/design-system.md, "Night"; plan R0, screen 4): Horizon, Vanilla
//! (Minecraft: Java Edition as Mojang ships it) and Modded (with Fabric or Quilt). A Java kind picks
//! its version from Mojang's public list (`minecraft_versions`), fetched once when the dialog first
//! needs it; the game itself comes with the first Play, for an account that owns it.
use super::*;
use crate::plate::alpha;
use crate::shell::{c, Action};
use horizon_launcher::loaders::{self, Kind as Loader};
use horizon_launcher::minecraft_versions::{Kind as VersionKind, Versions};
use servers_page::Field;
use theme::night;

/// Rows the version list shows at most: the search finds the others.
const ROWS: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewKind {
    Horizon,
    Vanilla,
    Modded,
}

/// What a new Java instance plays.
#[derive(Clone, Debug)]
pub(super) struct JavaChoice {
    pub(super) kind: NewKind,
    pub(super) loader: Loader,
    /// None: the newest release.
    pub(super) version: Option<String>,
    pub(super) snapshots: bool,
    pub(super) old: bool,
}

impl Default for JavaChoice {
    fn default() -> Self {
        JavaChoice {
            kind: NewKind::Horizon,
            loader: Loader::Fabric,
            version: None,
            snapshots: false,
            old: false,
        }
    }
}

impl JavaChoice {
    /// The Minecraft version it makes: the chosen one, else the newest release.
    pub(super) fn minecraft(&self, versions: Option<&Versions>) -> Option<String> {
        self.version
            .clone()
            .or_else(|| versions.map(|v| v.latest_release.clone()))
    }

    /// The name it gets when none is typed: "Vanilla 1.21.4", "Fabric 1.21.4".
    pub(super) fn default_name(&self, minecraft: &str) -> String {
        match self.kind {
            NewKind::Modded => format!("{} {minecraft}", self.loader.name()),
            _ => format!("Vanilla {minecraft}"),
        }
    }
}

/// `base`, or "base 2", "base 3"… when an instance already has it (names ignore case, as
/// `instances::create` does).
fn free_name(base: String, taken: &[&str]) -> String {
    let free = |n: &String| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(free)
        .unwrap_or(base)
}

/// Mojang's list as the dialog has it.
#[derive(Default)]
pub(super) enum VersionList {
    #[default]
    NotAsked,
    Loading,
    Ready(Versions),
    Failed(String),
}

impl Launcher {
    fn java_choice(&mut self) -> Option<&mut JavaChoice> {
        self.instances.dialog.as_mut().map(|d| &mut d.java)
    }

    /// The name a new instance gets for what it plays now (the mockup's `defaultName`): "My
    /// Horizon", "Vanilla 1.21.4", "Fabric 1.21.4", numbered past the ones already taken.
    fn default_name(&self) -> Option<String> {
        let j = &self.instances.dialog.as_ref()?.java;
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        let base = match j.kind {
            NewKind::Horizon => tr("My Horizon").to_string(),
            _ => j.default_name(&j.minecraft(versions)?),
        };
        let taken: Vec<&str> = self
            .instances
            .list
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        Some(free_name(base, &taken))
    }

    /// Changes what the new instance plays. Its name follows while it is the one the dialog gave
    /// (or none), never one the player typed.
    pub(super) fn change(&mut self, f: impl FnOnce(&mut JavaChoice)) {
        let before = self.default_name();
        if let Some(j) = self.java_choice() {
            f(j);
        }
        // An instance being edited keeps its name.
        if self
            .instances
            .dialog
            .as_ref()
            .is_some_and(|d| d.editing.is_some())
        {
            return;
        }
        let typed = self.servers.value(Field::InstanceName).trim().to_string();
        if typed.is_empty() || Some(&typed) == before.as_ref() {
            let after = self.default_name().unwrap_or_default();
            self.servers.text(Field::InstanceName).set(after);
        }
    }

    /// Picks the dialog's kind; a Java one fetches Mojang's list the first time.
    pub(super) fn pick_kind(&mut self, kind: NewKind, cx: &mut Context<Self>) {
        self.change(|j| j.kind = kind);
        if kind != NewKind::Horizon {
            self.load_versions(cx);
        }
    }

    /// Asks Mojang for its versions, the first time the dialog needs them.
    pub(super) fn load_versions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.instances.versions, VersionList::NotAsked) {
            return;
        }
        self.instances.versions = VersionList::Loading;
        let task = cx
            .background_executor()
            .spawn(async { horizon_launcher::minecraft_versions::load() });
        cx.spawn(async move |this, cx| {
            let list = task.await;
            let _ = this.update(cx, |this, cx| {
                let before = this.servers.value(Field::InstanceName).trim().is_empty();
                this.instances.versions = match list {
                    Ok(v) => VersionList::Ready(v),
                    Err(e) => VersionList::Failed(e),
                };
                // The newest release is known now: so is the name.
                if before {
                    this.change(|_| {});
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The tabs over a new instance's dialog: Horizon, Vanilla, Modded.
    pub(super) fn kind_tabs(&self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let now = self.instances.dialog.as_ref().map(|d| d.java.kind);
        div().flex().gap(px(4.)).children(
            [
                (NewKind::Horizon, "Horizon"),
                (NewKind::Vanilla, "Vanilla"),
                (NewKind::Modded, "Modded"),
            ]
            .map(|(kind, label)| {
                shell::tab(
                    format!("new-kind-{label}"),
                    tr(label),
                    now == Some(kind),
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.pick_kind(kind, cx);
                    cx.notify();
                }))
            }),
        )
    }

    /// A Java kind's part of the dialog: the version (search, kinds, list), the loader for
    /// Modded, and where the game comes from.
    pub(super) fn java_choice_rows(&mut self, cx: &mut Context<Self>) -> Div {
        let t = self.theme;
        let n = t.night;
        let Some(choice) = self.instances.dialog.as_ref().map(|d| d.java.clone()) else {
            return div();
        };
        let heading = |s: &str| {
            shell::text(night::CAPTION, c(n.muted))
                .font_weight(FontWeight::NORMAL)
                .child(tr(s).to_string())
        };
        let toggle = |id: &'static str, label: &'static str, on: bool| {
            shell::action(id, if on { Action::Raised } else { Action::Quiet }, &t)
                .h(px(26.))
                .px(px(10.))
                .child(shell::label(tr(label).to_string()))
        };
        let search = self
            .servers
            .value(Field::InstanceVersion)
            .trim()
            .to_lowercase();
        let chosen = choice.minecraft(match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        });
        let list: AnyElement = match &self.instances.versions {
            VersionList::NotAsked | VersionList::Loading => shell::text(night::UI, c(n.muted))
                .child(tr("Asking Mojang for its versions…").to_string())
                .into_any_element(),
            VersionList::Failed(e) => shell::text(night::UI, c(n.danger))
                .child(e.clone())
                .into_any_element(),
            VersionList::Ready(versions) => {
                let rows: Vec<_> = versions
                    .of(choice.snapshots, choice.old)
                    .filter(|v| search.is_empty() || v.id.to_lowercase().contains(&search))
                    .take(ROWS)
                    .map(|v| {
                        let selected = chosen.as_deref() == Some(v.id.as_str());
                        let id = v.id.clone();
                        let kind = match v.kind {
                            VersionKind::Release => "Release",
                            VersionKind::Snapshot => "Snapshot",
                            VersionKind::OldBeta => "Beta",
                            VersionKind::OldAlpha => "Alpha",
                        };
                        shell::row(format!("mc-{}", v.id), selected, &t)
                            .flex_none()
                            .h(px(32.))
                            .px(px(12.))
                            .child(
                                shell::text(night::UI, c(n.ink))
                                    .relative()
                                    .w(px(120.))
                                    .font_weight(FontWeight::NORMAL)
                                    .child(v.id.clone()),
                            )
                            .child(shell::text(night::CAPTION, c(n.muted)).relative().child(
                                format!("{} · {}", tr(kind), v.released.get(..10).unwrap_or("")),
                            ))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.change(|j| j.version = Some(id.clone()));
                                cx.notify();
                            }))
                    })
                    .collect();
                if rows.is_empty() {
                    shell::text(night::UI, c(n.muted))
                        .child(tr("No version matches.").to_string())
                        .into_any_element()
                } else {
                    div()
                        .id("mc-versions")
                        .flex()
                        .flex_col()
                        .h(px(6. * 32. + 8.))
                        .p(px(4.))
                        .overflow_y_scroll()
                        .rounded(px(night::RADIUS_CONTROL))
                        .border_1()
                        .border_color(alpha(n.stroke, n.stroke_alpha))
                        .children(rows)
                        .into_any_element()
                }
            }
        };
        let filters = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(self.field(Field::InstanceVersion, tr("Search versions"), 240., cx))
            .child(
                toggle("mc-snapshots", "Snapshots", choice.snapshots).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.change(|j| j.snapshots = !j.snapshots);
                        cx.notify();
                    },
                )),
            )
            .child(toggle("mc-old", "Old", choice.old).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.change(|j| j.old = !j.old);
                    cx.notify();
                },
            )));
        let mut rows = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(heading("Minecraft version"))
            .child(filters)
            .child(list);
        if choice.kind == NewKind::Modded {
            let scale = self.scale;
            let logos = [Loader::Fabric, Loader::Quilt].map(|l| (l, self.loader_logo(l.id(), 24.)));
            let card = |loader: Loader| {
                let logo = logos
                    .iter()
                    .find(|(l, _)| *l == loader)
                    .and_then(|(_, i)| i.clone());
                let on = choice.loader == loader;
                shell::action(
                    SharedString::from(format!("loader-{}", loader.id())),
                    Action::Raised,
                    &t,
                )
                .h(px(48.))
                .w(px(180.))
                .justify_start()
                .when(on, |d| d.border_1().border_color(c(n.accent_hi)))
                .gap(px(10.))
                .children(logo.map(|logo| instance_icons::centred(logo, 24., scale).relative()))
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .child(loader.name())
                        .child(
                            shell::text(night::CAPTION, c(n.muted))
                                .child(tr("newest stable version").to_string()),
                        ),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.change(|j| j.loader = loader);
                    cx.notify();
                }))
            };
            rows = rows.child(heading("Loader")).child(
                div()
                    .flex()
                    .gap(px(10.))
                    .child(card(Loader::Fabric))
                    .child(card(Loader::Quilt)),
            );
        }
        rows.child(shell::text(night::CAPTION, c(n.muted)).child(
            tr("Play downloads the game from Mojang for your account the first time, with the Java it needs.")
                .to_string(),
        ))
    }
}

/// Makes a Java instance: `name`, playing `minecraft`, with the newest stable loader for Modded
/// (asked of its service now, so a version it doesn't support says so here).
pub(super) fn create_java(
    choice: &JavaChoice,
    name: &str,
    minecraft: &str,
) -> Result<instances::Instance, String> {
    let loader = (choice.kind == NewKind::Modded)
        .then(|| loaders::choose(choice.loader.id(), minecraft, None))
        .transpose()?;
    let made = instances::create_java(name, minecraft)?;
    match loader {
        Some(loader) => instances::edit(&made.id, |m| {
            m.java.get_or_insert_with(Default::default).loader = Some(loader)
        }),
        None => Ok(made),
    }
}

/// `to`, with the newest stable version of a loader it names without one (asked of its service
/// now, so a Minecraft version the loader doesn't support says so here).
pub(super) fn with_loader(to: instances::Plays) -> Result<instances::Plays, String> {
    match to {
        instances::Plays::Java {
            minecraft,
            loader: Some(l),
        } if l.version.is_empty() => Ok(instances::Plays::Java {
            loader: Some(loaders::choose(l.kind.id(), &minecraft, None)?),
            minecraft,
        }),
        other => Ok(other),
    }
}

impl Launcher {
    pub(super) fn draft_minecraft(&self) -> Option<String> {
        let versions = match &self.instances.versions {
            VersionList::Ready(v) => Some(v),
            _ => None,
        };
        self.instances.dialog.as_ref()?.java.minecraft(versions)
    }

    pub(super) fn draft_horizon(&self) -> Option<String> {
        let pin = self.instances.dialog.as_ref()?.pin.clone();
        pin.or_else(|| self.followed_version())
    }

    fn followed_version(&self) -> Option<String> {
        self.launcher_version()
            .cloned()
            .or_else(|| self.horizon_choices().into_iter().next())
    }

    pub(super) fn horizon_choices(&self) -> Vec<String> {
        let releases = match &self.releases {
            Load::Ready(r) => r.as_slice(),
            _ => &[],
        };
        horizon_choices(releases, &self.installed)
    }

    pub(super) fn release_to_install(&self, tag: &str) -> Option<Release> {
        let Load::Ready(releases) = &self.releases else {
            return None;
        };
        (!self.installed.iter().any(|t| t == tag))
            .then(|| releases.iter().find(|r| r.tag == tag && r.asset.is_some()))
            .flatten()
            .cloned()
    }

    pub(super) fn draft_plays(&self, editing: &instances::Instance) -> Option<instances::Plays> {
        let d = self.instances.dialog.as_ref()?;
        if d.java.kind == NewKind::Horizon {
            return Some(instances::Plays::Horizon);
        }
        let minecraft = self.draft_minecraft()?;
        let loader = (d.java.kind == NewKind::Modded).then(|| {
            editing
                .java
                .as_ref()
                .filter(|j| j.minecraft == minecraft)
                .and_then(|j| j.loader.clone())
                .filter(|l| l.kind == d.java.loader)
                .unwrap_or(loaders::Loader {
                    kind: d.java.loader,
                    version: String::new(),
                })
        });
        Some(instances::Plays::Java { minecraft, loader })
    }

    pub(super) fn switch_notice(&self, editing: &instances::Instance) -> Option<Div> {
        let to = self.draft_plays(editing)?;
        if to == instances::Plays::of(editing) {
            return None;
        }
        let horizon = backend::game_version(&self.draft_horizon().unwrap_or_default());
        let lines: Vec<String> = instances::unkept(editing, &to, &horizon)
            .into_iter()
            .map(|u| match u {
                instances::Unkept::Mods { made_for, .. } => trf(
                    "Its mods made for {loader} stay in its folder but won't load.",
                    &[("loader", &made_for)],
                ),
                instances::Unkept::ModsVersion { from, to, .. } => trf(
                    "Its mods made for Minecraft {from} stay in its folder; some may not load in {to}.",
                    &[("from", &from), ("to", &to)],
                ),
                instances::Unkept::Worlds { from, to, .. } => trf(
                    "Its worlds were saved by Minecraft {from}: {to} upgrades them for good, and can't open them if it is older.",
                    &[("from", &from), ("to", &to)],
                ),
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        let t = self.theme;
        Some(
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap(px(6.))
                .p(px(12.))
                .border_1()
                .border_color(t.warn)
                .child(
                    div()
                        .text_color(t.warn)
                        .child(tr("What this change can't keep")),
                )
                .children(
                    lines
                        .into_iter()
                        .map(|l| div().text_color(t.ink_soft).child(l)),
                )
                .child(div().text_color(t.ink_soft).child(tr(
                    "Save changes switches it. Nothing in its folder is deleted.",
                ))),
        )
    }
}

pub(super) fn horizon_choices(releases: &[Release], installed: &[String]) -> Vec<String> {
    let published = releases
        .iter()
        .filter(|r| r.nightly.is_some() && r.asset.is_some())
        .map(|r| &r.tag);
    let mut out: Vec<String> = Vec::new();
    for tag in published.chain(installed) {
        if !out.contains(tag) {
            out.push(tag.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, nightly: bool, asset: bool) -> Release {
        Release {
            tag: tag.into(),
            name: String::new(),
            body: String::new(),
            date: String::new(),
            time: String::new(),
            asset: asset.then(|| ("horizon.tar.gz".into(), 1)),
            prerelease: false,
            nightly: nightly.then(|| "nightly".into()),
        }
    }

    #[test]
    fn the_dialog_offers_every_published_nightly_then_what_only_this_computer_has() {
        let releases = [
            release("nightly-2026-11-03-aaaaaaa", true, true),
            release("nightly-2026-11-02-bbbbbbb", true, false),
            release("nightly-2026-11-01-ccccccc", true, true),
            release("v0.2.1", false, true),
        ];
        let installed = [
            "nightly-2026-11-01-ccccccc".to_string(),
            "nightly-2026-10-20-ddddddd".to_string(),
        ];
        assert_eq!(
            horizon_choices(&releases, &installed),
            [
                "nightly-2026-11-03-aaaaaaa",
                "nightly-2026-11-01-ccccccc",
                "nightly-2026-10-20-ddddddd",
            ]
        );
        assert_eq!(horizon_choices(&[], &installed), installed);
    }

    #[test]
    fn a_new_java_instance_is_named_after_what_it_plays() {
        let mut choice = JavaChoice {
            kind: NewKind::Vanilla,
            ..Default::default()
        };
        assert_eq!(choice.default_name("1.21.4"), "Vanilla 1.21.4");
        choice.kind = NewKind::Modded;
        choice.loader = Loader::Quilt;
        assert_eq!(choice.default_name("1.21.4"), "Quilt 1.21.4");
        assert_eq!(choice.minecraft(None), None);
        choice.version = Some("1.20.1".into());
        assert_eq!(choice.minecraft(None).as_deref(), Some("1.20.1"));
    }

    #[test]
    fn a_default_name_skips_the_ones_taken() {
        assert_eq!(free_name("My Horizon".into(), &["Horizon"]), "My Horizon");
        assert_eq!(
            free_name("My Horizon".into(), &["my horizon", "My Horizon 2"]),
            "My Horizon 3"
        );
    }
}
