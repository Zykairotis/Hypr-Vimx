#![cfg(feature = "opencv-backend")]
use crate::backends::{Backend, BackendResult};
use crate::config::Config;
use crate::hints::Child;
use crate::window_system::WindowSystem;
use anyhow::{Context, Result, anyhow};
use opencv::core::{self, Point, Size};
use opencv::imgcodecs;
use opencv::imgproc;
use opencv::prelude::*;
use std::process::Command;
use tempfile::NamedTempFile;

pub struct OpenCvBackend {
    cfg: Config,
    window_system: WindowSystem,
}

impl OpenCvBackend {
    pub fn new(cfg: Config, window_system: WindowSystem) -> Self {
        Self { cfg, window_system }
    }

    fn screenshot(&self) -> Result<Mat> {
        // Only use window-specific capture if explicitly enabled (faster but may miss elements)
        if std::env::var("HINTSX_WINDOW_CAPTURE").is_ok()
            && self.cfg.overlay_target == crate::config::OverlayTarget::Window
        {
            if let Some((x, y, w, h)) = self.get_active_window_geometry() {
                // Use grim with specific geometry for faster capture
                let geometry = format!("{},{} {}x{}", x, y, w, h);
                let output = Command::new("grim")
                    .args(["-g", &geometry, "-t", "ppm", "-"]) // PPM is faster than PNG
                    .output()?;

                if output.status.success() && !output.stdout.is_empty() {
                    // Decode PPM directly from memory
                    let img_vec = opencv::core::Vector::<u8>::from_iter(output.stdout.into_iter());
                    let mat = imgcodecs::imdecode(&img_vec, imgcodecs::IMREAD_COLOR)?;
                    if !mat.empty() {
                        return Ok(mat);
                    }
                }
            }
        }

        // Try fast stdout capture first (avoids file I/O)
        match self.window_system.window_system_type {
            crate::window_system::WindowSystemType::Wayland => {
                // Try grim with PPM to stdout (much faster than PNG)
                let output = Command::new("grim")
                    .args(["-t", "ppm", "-"]) // PPM format to stdout
                    .output();

                if let Ok(output) = output {
                    if output.status.success() && !output.stdout.is_empty() {
                        let img_vec =
                            opencv::core::Vector::<u8>::from_iter(output.stdout.into_iter());
                        let mat = imgcodecs::imdecode(&img_vec, imgcodecs::IMREAD_COLOR)?;
                        if !mat.empty() {
                            return Ok(mat);
                        }
                    }
                }
            }
            crate::window_system::WindowSystemType::X11 => {
                // Try shotgun with PPM to stdout
                let output = Command::new("shotgun")
                    .args(["-f", "ppm", "-"]) // PPM format to stdout
                    .output();

                if let Ok(output) = output {
                    if output.status.success() && !output.stdout.is_empty() {
                        let img_vec =
                            opencv::core::Vector::<u8>::from_iter(output.stdout.into_iter());
                        let mat = imgcodecs::imdecode(&img_vec, imgcodecs::IMREAD_COLOR)?;
                        if !mat.empty() {
                            return Ok(mat);
                        }
                    }
                }
            }
        }

        // Fallback to file-based capture if stdout fails
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();
        let path_str = path.to_str().unwrap();

        let commands: Vec<(&str, Vec<&str>)> = match self.window_system.window_system_type {
            crate::window_system::WindowSystemType::Wayland => {
                vec![("wayshot", vec!["-f"]), ("grim", vec![])]
            }
            crate::window_system::WindowSystemType::X11 => {
                vec![("shotgun", vec![]), ("maim", vec![])]
            }
        };

        let mut last_error = None;

        for (cmd, args_prefix) in commands {
            let mut cmd_build = Command::new(cmd);
            cmd_build.args(&args_prefix);
            cmd_build.arg(path_str);

            match cmd_build.status() {
                Ok(status) => {
                    if status.success() {
                        let mat = imgcodecs::imread(path_str, imgcodecs::IMREAD_COLOR)
                            .context("read screenshot into mat")?;
                        return Ok(mat);
                    } else {
                        last_error = Some(anyhow!("{} failed with status {:?}", cmd, status));
                    }
                }
                Err(e) => {
                    // Command not found or failed to launch
                    last_error = Some(anyhow!("failed to execute {}: {}", cmd, e));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("no suitable screenshot tool found")))
    }

    fn get_active_window_geometry(&self) -> Option<(i32, i32, i32, i32)> {
        if self.window_system.window_system_type == crate::window_system::WindowSystemType::Wayland
        {
            self.window_system
                .get_active_window_geometry_wayland()
                .or_else(|| self.window_system.get_active_window_geometry_x11())
        } else {
            self.window_system.get_active_window_geometry_x11()
        }
    }
}

impl Backend for OpenCvBackend {
    fn name(&self) -> &'static str {
        "opencv"
    }

    fn get_children(&mut self) -> Result<BackendResult> {
        let cfg = self.cfg.backends.opencv.clone();
        let img = self.screenshot()?;
        let mut gray = Mat::default();
        imgproc::cvt_color(
            &img,
            &mut gray,
            imgproc::COLOR_BGR2GRAY,
            0,
            core::AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;

        let mut edges = Mat::default();
        imgproc::canny(
            &gray,
            &mut edges,
            cfg.canny_min_val,
            cfg.canny_max_val,
            3,
            false,
        )?;

        let kernel = imgproc::get_structuring_element(
            imgproc::MORPH_RECT,
            Size::new(cfg.kernel_size, cfg.kernel_size),
            Point::new(-1, -1),
        )?;
        let mut dilated = Mat::default();
        imgproc::dilate(
            &edges,
            &mut dilated,
            &kernel,
            Point::new(-1, -1),
            1,
            core::BORDER_DEFAULT,
            imgproc::morphology_default_border_value()?,
        )?;

        let mut contours = opencv::types::VectorOfVectorOfPoint::new();
        imgproc::find_contours(
            &dilated,
            &mut contours,
            imgproc::RETR_LIST,
            imgproc::CHAIN_APPROX_SIMPLE,
            Point::new(0, 0),
        )?;

        let mut children = Vec::new();
        for contour in contours {
            let rect = imgproc::bounding_rect(&contour)?;
            // filter tiny rects
            if rect.width < 5 || rect.height < 5 {
                continue;
            }
            children.push(Child {
                absolute_x: rect.x,
                absolute_y: rect.y,
                width: rect.width,
                height: rect.height,
            });
        }

        let mut focus_extents = None;
        if self.cfg.overlay_target == crate::config::OverlayTarget::Window {
            let extents = if self.window_system.window_system_type
                == crate::window_system::WindowSystemType::Wayland
            {
                self.window_system
                    .get_active_window_geometry_wayland()
                    .or_else(|| self.window_system.get_active_window_geometry_x11())
            } else {
                self.window_system.get_active_window_geometry_x11()
            };

            if let Some((fx, fy, fw, fh)) = extents {
                focus_extents = Some((fx, fy, fw, fh));
                // Filter children to be inside the active window
                children.retain(|c| {
                    c.absolute_x >= fx
                        && c.absolute_y >= fy
                        && (c.absolute_x + c.width) <= (fx + fw)
                        && (c.absolute_y + c.height) <= (fy + fh)
                });
            }
        }

        if children.is_empty() {
            Err(anyhow!("opencv backend found zero contours"))
        } else {
            Ok(BackendResult {
                children,
                focus_extents,
            })
        }
    }
}
