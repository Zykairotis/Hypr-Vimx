#![cfg(feature = "atspi-backend")]
use crate::backends::{Backend, BackendResult};
use crate::config::{Config, OverlayTarget};
use crate::hints::Child;
use crate::window_system::WindowSystem;
use anyhow::{Result, anyhow};

use atspi::connection::AccessibilityConnection;
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::{CoordType, Role, State};
use futures::future::join_all;
use tokio::runtime::Runtime;
use zbus::zvariant::OwnedObjectPath;

pub struct AtspiBackend {
    cfg: Config,
    window_system: WindowSystem,
    rt: Runtime,
}

impl AtspiBackend {
    pub fn new(cfg: Config, window_system: WindowSystem) -> Self {
        Self {
            cfg,
            window_system,
            rt: Runtime::new().expect("tokio runtime"),
        }
    }

    async fn collect_children(&self) -> Result<(Vec<Child>, Option<(i32, i32, i32, i32)>)> {
        let conn = AccessibilityConnection::new().await?;

        let bus = conn.connection();

        let root = AccessibleProxy::builder(bus)
            .path(OwnedObjectPath::try_from(
                "/org/a11y/atspi/accessible/root",
            )?)?
            .build()
            .await?;

        let mut out = Vec::new();
        let mut focus_extents: Option<(i32, i32, i32, i32)> = None;
        if self.cfg.overlay_target == OverlayTarget::Window {
            if let Some((focused_path, extents)) = self.find_focused_window(&root, bus).await? {
                focus_extents = Some(extents);
                self.walk_iterative(focused_path, &mut out, bus, focus_extents)
                    .await?;
            } else {
                log::warn!(
                    "atspi backend: no focused window found via atspi; trying native/xdotool fallback"
                );

                let fallback_extents = if self.window_system.window_system_type
                    == crate::window_system::WindowSystemType::Wayland
                {
                    self.window_system
                        .get_active_window_geometry_wayland()
                        .or_else(|| self.window_system.get_active_window_geometry_x11())
                } else {
                    self.window_system.get_active_window_geometry_x11()
                };

                if let Some(extents) = fallback_extents {
                    focus_extents = Some(extents);
                    self.walk_iterative(
                        root.inner().path().to_owned().into(),
                        &mut out,
                        bus,
                        focus_extents,
                    )
                    .await?;
                } else {
                    log::warn!("atspi backend: xdotool fallback failed; falling back to full tree");
                    self.walk_iterative(
                        root.inner().path().to_owned().into(),
                        &mut out,
                        bus,
                        focus_extents,
                    )
                    .await?;
                }
            }
        } else {
            self.walk_iterative(
                root.inner().path().to_owned().into(),
                &mut out,
                bus,
                focus_extents,
            )
            .await?;
        }

        if out.is_empty() {
            // If no children found with focus filtering, try without filtering
            self.walk_iterative(
                root.inner().path().to_owned().into(),
                &mut out,
                bus,
                None, // No focus filtering
            )
            .await?;
        }

        if out.is_empty() {
            Err(anyhow!("atspi backend found zero children"))
        } else {
            Ok((out, focus_extents))
        }
    }

    async fn find_focused_window(
        &self,
        root: &AccessibleProxy<'_>,
        bus: &zbus::Connection,
    ) -> Result<Option<(OwnedObjectPath, (i32, i32, i32, i32))>> {
        let mut first_window: Option<(OwnedObjectPath, (i32, i32, i32, i32))> = None;
        let mut focused_node: Option<OwnedObjectPath> = None;

        let children_refs = root.get_children().await.unwrap_or_default();

        // Optimization: Check all apps in parallel to find the active one
        let state_futures = children_refs.iter().map(|child_ref| async move {
            if let Ok(proxy) = AccessibleProxy::builder(bus).path(child_ref.path.clone()) {
                if let Ok(proxy) = proxy.build().await {
                    if let Ok(state) = proxy.get_state().await {
                        return Some((child_ref.path.clone(), state));
                    }
                }
            }
            None
        });

        let states = join_all(state_futures).await;
        let active_app = states
            .into_iter()
            .flatten()
            .find(|(_, s)| s.contains(State::Active));

        let mut stack: Vec<OwnedObjectPath> = if let Some((path, _)) = active_app {
            vec![path]
        } else {
            children_refs.into_iter().map(|c| c.path).collect()
        };

        while let Some(path) = stack.pop() {
            let accessible = match AccessibleProxy::builder(bus)
                .path(path.clone())?
                .build()
                .await
            {
                Ok(a) => a,
                Err(_) => continue,
            };

            let state_set = accessible.get_state().await.unwrap_or_default();
            let role = accessible.get_role().await.unwrap_or(Role::Invalid);
            let focused = state_set.contains(State::Focused) || state_set.contains(State::Active);
            let windowish = matches!(
                role,
                Role::Frame
                    | Role::Window
                    | Role::Dialog
                    | Role::Alert
                    | Role::DesktopFrame
                    | Role::InternalFrame
                    | Role::Application
            );

            if focused && windowish {
                if let Ok(component) = ComponentProxy::builder(bus)
                    .path(path.clone())?
                    .build()
                    .await
                {
                    if let Ok((x, y, w, h)) = component.get_extents(CoordType::Screen).await {
                        return Ok(Some((path, (x, y, w, h))));
                    }
                }
                return Ok(Some((path, (0, 0, 0, 0))));
            }

            if focused && focused_node.is_none() {
                focused_node = Some(path.clone());
            }

            if windowish && first_window.is_none() {
                if let Ok(component) = ComponentProxy::builder(bus)
                    .path(path.clone())?
                    .build()
                    .await
                {
                    if let Ok((x, y, w, h)) = component.get_extents(CoordType::Screen).await {
                        first_window = Some((path.clone(), (x, y, w, h)));
                    }
                }
            }

            if let Ok(children) = accessible.get_children().await {
                stack.extend(children.into_iter().map(|c| c.path));
            }
        }

        // If we saw a focused descendant but not a focused window, climb parents to find its windowish ancestor
        if let Some(node_path) = focused_node {
            let mut current_path = node_path;
            loop {
                let accessible = AccessibleProxy::builder(bus)
                    .path(current_path.clone())?
                    .build()
                    .await?;
                let role = accessible.get_role().await.unwrap_or(Role::Invalid);
                let windowish = matches!(
                    role,
                    Role::Frame
                        | Role::Window
                        | Role::Dialog
                        | Role::Alert
                        | Role::DesktopFrame
                        | Role::InternalFrame
                        | Role::Application
                );
                if windowish {
                    if let Ok(component) = ComponentProxy::builder(bus)
                        .path(current_path.clone())?
                        .build()
                        .await
                    {
                        if let Ok((x, y, w, h)) = component.get_extents(CoordType::Screen).await {
                            return Ok(Some((current_path, (x, y, w, h))));
                        }
                    }
                    return Ok(Some((current_path, (0, 0, 0, 0))));
                }

                // climb to parent; break if none
                if let Ok(parent_ref) = accessible.parent().await {
                    current_path = parent_ref.path;
                } else {
                    break;
                }
            }
        }

        Ok(first_window)
    }

    async fn walk_iterative(
        &self,
        start_path: OwnedObjectPath,
        out: &mut Vec<Child>,
        bus: &zbus::Connection,
        focus_extents: Option<(i32, i32, i32, i32)>,
    ) -> Result<()> {
        let mut current_level = vec![start_path];
        let mut visited = std::collections::HashSet::new(); // Restore cycle detection

        // Limit depth to avoid infinite loops or too deep traversal
        let mut depth = 0;
        const MAX_DEPTH: usize = 50; // Restore original depth

        while !current_level.is_empty() && depth < MAX_DEPTH {
            depth += 1;

            // Filter out visited paths to prevent cycles
            current_level.retain(|p| visited.insert(p.clone()));
            if current_level.is_empty() {
                break;
            }

            // Process current level in parallel
            let futures = current_level.iter().map(|path| async move {
                let mut result_children = Vec::new();
                let mut result_child = None;

                // Skip null path explicitly
                if path.as_str() == "/org/a11y/atspi/null" {
                    return (result_child, result_children);
                }

                // Try to build accessible proxy
                if let Ok(proxy) = AccessibleProxy::builder(bus).path(path.clone()) {
                    if let Ok(proxy) = proxy.build().await {
                        // Get children
                        if let Ok(children) = proxy.get_children().await {
                            result_children = children.into_iter().map(|c| c.path).collect();
                        }

                        // Get extents (via Component interface)
                        // Not all accessibles implement Component, so this might fail/return error, which is fine
                        if let Ok(component) = ComponentProxy::builder(bus).path(path.clone()) {
                            if let Ok(component) = component.build().await {
                                if let Ok((x, y, w, h)) =
                                    component.get_extents(CoordType::Screen).await
                                {
                                    if w > 0 && h > 0 {
                                        result_child = Some((x, y, w, h));
                                    }
                                }
                            }
                        }
                    }
                }
                (result_child, result_children)
            });

            let results = join_all(futures).await;

            current_level = Vec::new();

            for (child_opt, children_paths) in results {
                if let Some((x, y, w, h)) = child_opt {
                    let inside_focus = focus_extents.map_or(true, |(fx, fy, fw, fh)| {
                        x >= fx && y >= fy && (x + w) <= (fx + fw) && (y + h) <= (fy + fh)
                    });
                    if inside_focus {
                        out.push(Child {
                            absolute_x: x,
                            absolute_y: y,
                            width: w,
                            height: h,
                        });
                    }
                }
                current_level.extend(children_paths);
            }
        }
        Ok(())
    }
}

impl Backend for AtspiBackend {
    fn name(&self) -> &'static str {
        "atspi"
    }

    fn get_children(&mut self) -> Result<BackendResult> {
        let (children, focus_extents) = self.rt.block_on(self.collect_children())?;
        Ok(BackendResult {
            children,
            focus_extents,
        })
    }
}
