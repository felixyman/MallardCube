pub mod config;
#[allow(clippy::module_inception)]
// project/project.rs predates the module split; lib.rs re-exports depend on the path
pub mod project;
