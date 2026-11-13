use std::path::PathBuf;

use typed_env::{Envar, EnvarDef};

pub static COMPOTE_IGNORE: &[&str] = &["CONFIG"];

pub static CONFIG_PATH: Envar<PathBuf> = Envar::on_demand("TNTOP_CONFIG", || EnvarDef::Unset);
