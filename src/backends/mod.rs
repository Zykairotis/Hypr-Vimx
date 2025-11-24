use crate::config::Config;
use crate::hints::Child;
use crate::window_system::WindowSystem;
use anyhow::Result;

pub mod atspi_backend;
#[cfg(feature = "opencv-backend")]
pub mod opencv_backend;

#[derive(Debug, Clone)]
pub struct BackendResult {
    pub children: Vec<Child>,
    pub focus_extents: Option<(i32, i32, i32, i32)>,
}

pub trait Backend {
    fn name(&self) -> &'static str;
    fn get_children(&mut self) -> Result<BackendResult>;
}

pub fn build_backends(cfg: &Config, window_system: &WindowSystem) -> Vec<Box<dyn Backend + Send>> {
    let mut list: Vec<Box<dyn Backend + Send>> = Vec::new();
    for name in &cfg.backends.enable {
        match name.as_str() {
            "atspi" => {
                list.push(Box::new(atspi_backend::AtspiBackend::new(
                    cfg.clone(),
                    window_system.clone(),
                )));
            }
            #[cfg(feature = "opencv-backend")]
            "opencv" => {
                list.push(Box::new(opencv_backend::OpenCvBackend::new(
                    cfg.clone(),
                    window_system.clone(),
                )));
            }
            _ => {}
        }
    }
    list
}
