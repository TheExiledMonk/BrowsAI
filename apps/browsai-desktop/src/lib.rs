//! Toolkit-neutral visual shell model for optional desktop rendering.

use browsai_layout_observer::LayoutSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SurfaceId(Uuid);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Surface {
    Window {
        width: u32,
        height: u32,
    },
    Tab {
        title: String,
        url: String,
    },
    NavigationControls {
        back_enabled: bool,
        forward_enabled: bool,
        reload_enabled: bool,
    },
    Dialog {
        title: String,
        message: String,
    },
    PermissionPrompt {
        origin: String,
        permission: String,
    },
    Confirmation {
        request_id: String,
        summary: String,
    },
    Download {
        filename: String,
        progress: f32,
    },
    Takeover {
        reason: String,
    },
    Diagnostics {
        message: String,
        level: String,
    },
    AccessibilityNotice {
        role: String,
        label: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SurfaceLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub z_index: u32,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RenderSurface {
    pub id: SurfaceId,
    pub surface: Surface,
    pub layout: SurfaceLayout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaintCommand {
    pub node_id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z_index: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CompositorFrame {
    pub generation: u64,
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub commands: Vec<PaintCommand>,
}

#[derive(Clone, Debug, Default)]
pub struct DesktopShell {
    surfaces: BTreeMap<SurfaceId, Surface>,
    layouts: BTreeMap<SurfaceId, SurfaceLayout>,
    focused: Option<SurfaceId>,
    locale: String,
}

#[derive(Serialize, Deserialize)]
struct DesktopShellWire {
    surfaces: Vec<(SurfaceId, Surface)>,
    #[serde(default)]
    layouts: Vec<(SurfaceId, SurfaceLayout)>,
    focused: Option<SurfaceId>,
    #[serde(default = "default_locale")]
    locale: String,
}

fn default_locale() -> String {
    "en-US".into()
}

impl DesktopShell {
    /// Projects engine layout into an optional paint frame. Agent consumers do
    /// not depend on this frame; the shell may use it for visual rendering.
    pub fn compose_layout(layout: &LayoutSnapshot) -> CompositorFrame {
        let mut commands = layout
            .boxes
            .values()
            .filter(|layout| layout.visible && !layout.clipped)
            .map(|layout| PaintCommand {
                node_id: layout.node_id,
                x: layout.rect.x,
                y: layout.rect.y,
                width: layout.rect.width,
                height: layout.rect.height,
                z_index: layout.z_index,
            })
            .collect::<Vec<_>>();
        commands.sort_by_key(|command| (command.z_index, command.node_id));
        CompositorFrame {
            generation: layout.generation,
            viewport_width: layout.viewport.width,
            viewport_height: layout.viewport.height,
            commands,
        }
    }

    pub fn open(&mut self, surface: Surface) -> SurfaceId {
        let id = SurfaceId(Uuid::new_v4());
        let z_index = self
            .layouts
            .values()
            .map(|layout| layout.z_index)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let (width, height) = match &surface {
            Surface::Window { width, height } => (*width, *height),
            Surface::Tab { .. } => (1024, 640),
            Surface::NavigationControls { .. } => (640, 48),
            Surface::Dialog { .. }
            | Surface::PermissionPrompt { .. }
            | Surface::Confirmation { .. }
            | Surface::Takeover { .. }
            | Surface::Diagnostics { .. }
            | Surface::AccessibilityNotice { .. } => (480, 240),
            Surface::Download { .. } => (420, 120),
        };
        self.surfaces.insert(id, surface);
        self.layouts.insert(
            id,
            SurfaceLayout {
                x: 0,
                y: 0,
                width,
                height,
                z_index,
                visible: true,
            },
        );
        self.focused = Some(id);
        id
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn set_locale(&mut self, locale: impl Into<String>) {
        self.locale = locale.into();
    }
    pub fn close(&mut self, id: SurfaceId) -> Option<Surface> {
        let removed = self.surfaces.remove(&id);
        self.layouts.remove(&id);
        if self.focused == Some(id) {
            self.focused = self.surfaces.keys().next().copied();
        }
        removed
    }
    pub fn focus(&mut self, id: SurfaceId) -> bool {
        if self.surfaces.contains_key(&id) {
            self.focused = Some(id);
            true
        } else {
            false
        }
    }
    pub fn focused(&self) -> Option<(SurfaceId, &Surface)> {
        self.focused
            .and_then(|id| self.surfaces.get(&id).map(|surface| (id, surface)))
    }
    pub fn surfaces(&self) -> impl Iterator<Item = (SurfaceId, &Surface)> {
        self.surfaces.iter().map(|(id, surface)| (*id, surface))
    }

    pub fn layout(&self, id: SurfaceId) -> Option<SurfaceLayout> {
        self.layouts.get(&id).copied()
    }

    pub fn set_layout(&mut self, id: SurfaceId, layout: SurfaceLayout) -> bool {
        if !self.surfaces.contains_key(&id) {
            return false;
        }
        self.layouts.insert(id, layout);
        true
    }

    /// Returns visible surfaces in compositor order, with topmost surfaces last.
    pub fn render_plan(&self) -> Vec<RenderSurface> {
        let mut rendered = self
            .surfaces
            .iter()
            .filter_map(|(id, surface)| {
                let layout = self.layouts.get(id).copied()?;
                layout.visible.then(|| RenderSurface {
                    id: *id,
                    surface: surface.clone(),
                    layout,
                })
            })
            .collect::<Vec<_>>();
        rendered.sort_by_key(|surface| (surface.layout.z_index, surface.id));
        rendered
    }

    pub fn hit_test(&self, x: i32, y: i32) -> Option<SurfaceId> {
        self.render_plan()
            .into_iter()
            .rev()
            .find(|surface| {
                x >= surface.layout.x
                    && y >= surface.layout.y
                    && x < surface.layout.x.saturating_add(surface.layout.width as i32)
                    && y < surface
                        .layout
                        .y
                        .saturating_add(surface.layout.height as i32)
            })
            .map(|surface| surface.id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&DesktopShellWire {
            surfaces: self
                .surfaces
                .iter()
                .map(|(id, surface)| (*id, surface.clone()))
                .collect(),
            layouts: self
                .layouts
                .iter()
                .map(|(id, layout)| (*id, *layout))
                .collect(),
            focused: self.focused,
            locale: self.locale.clone(),
        })
    }

    pub fn from_json(value: &str) -> Result<Self, serde_json::Error> {
        let wire: DesktopShellWire = serde_json::from_str(value)?;
        let surfaces: BTreeMap<SurfaceId, Surface> = wire.surfaces.into_iter().collect();
        let layouts = wire
            .layouts
            .into_iter()
            .filter(|(id, _)| surfaces.contains_key(id))
            .collect();
        let focused = wire.focused.filter(|id| surfaces.contains_key(id));
        Ok(Self {
            surfaces,
            layouts,
            focused,
            locale: wire.locale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use browsai_layout_observer::{LayoutBox, Rect};
    #[test]
    fn shell_tracks_focus_and_confirmation_surfaces() {
        let mut shell = DesktopShell::default();
        let window = shell.open(Surface::Window {
            width: 800,
            height: 600,
        });
        let confirmation = shell.open(Surface::Confirmation {
            request_id: "r1".into(),
            summary: "Confirm payment".into(),
        });
        assert_eq!(shell.focused().unwrap().0, confirmation);
        assert!(shell.focus(window));
        assert_eq!(shell.focused().unwrap().0, window);
        assert!(matches!(
            shell.close(confirmation),
            Some(Surface::Confirmation { .. })
        ));
    }

    #[test]
    fn shell_builds_ordered_render_plan_and_hit_tests_surfaces() {
        let mut shell = DesktopShell::default();
        let window = shell.open(Surface::Window {
            width: 800,
            height: 600,
        });
        let dialog = shell.open(Surface::Dialog {
            title: "Notice".into(),
            message: "Hello".into(),
        });
        assert!(shell.set_layout(
            window,
            SurfaceLayout {
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                z_index: 1,
                visible: true,
            }
        ));
        assert!(shell.set_layout(
            dialog,
            SurfaceLayout {
                x: 100,
                y: 100,
                width: 200,
                height: 100,
                z_index: 2,
                visible: true,
            }
        ));
        assert_eq!(shell.render_plan().len(), 2);
        assert_eq!(shell.hit_test(150, 150), Some(dialog));
        assert_eq!(shell.hit_test(10, 10), Some(window));
    }

    #[test]
    fn shell_surfaces_and_focus_round_trip_for_recovery() {
        let mut shell = DesktopShell::default();
        let window = shell.open(Surface::Window {
            width: 800,
            height: 600,
        });
        shell.open(Surface::PermissionPrompt {
            origin: "https://example.test".into(),
            permission: "Camera".into(),
        });
        shell.focus(window);
        let restored = DesktopShell::from_json(&shell.to_json().unwrap()).unwrap();
        assert_eq!(restored.focused().unwrap().0, window);
        assert_eq!(restored.surfaces().count(), 2);
    }

    #[test]
    fn compositor_projects_visible_engine_layout_in_z_order() {
        let mut layout = LayoutSnapshot {
            viewport: Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            ..Default::default()
        };
        layout.insert(LayoutBox {
            node_id: 2,
            rect: Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 40.0,
            },
            visible: true,
            clipped: false,
            z_index: 3,
            pointer_events: true,
            disabled: false,
            provenance: vec![],
        });
        layout.insert(LayoutBox {
            node_id: 1,
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            visible: false,
            clipped: false,
            z_index: 1,
            pointer_events: false,
            disabled: false,
            provenance: vec![],
        });
        let frame = DesktopShell::compose_layout(&layout);
        assert_eq!(frame.generation, layout.generation);
        assert_eq!(frame.commands.len(), 1);
        assert_eq!(frame.commands[0].node_id, 2);
    }

    #[test]
    fn shell_exposes_navigation_diagnostics_accessibility_and_locale_state() {
        let mut shell = DesktopShell::default();
        shell.set_locale("fil-PH");
        shell.open(Surface::NavigationControls {
            back_enabled: true,
            forward_enabled: false,
            reload_enabled: true,
        });
        shell.open(Surface::Diagnostics {
            message: "renderer healthy".into(),
            level: "info".into(),
        });
        shell.open(Surface::AccessibilityNotice {
            role: "status".into(),
            label: "Page loaded".into(),
        });
        let restored = DesktopShell::from_json(&shell.to_json().unwrap()).unwrap();
        assert_eq!(restored.locale(), "fil-PH");
        assert_eq!(restored.surfaces().count(), 3);
    }
}
