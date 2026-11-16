use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphSymbol {
  Default,
  #[default]
  Braille,
  Block,
  Tty,
}

impl GraphSymbol {
  pub fn resolve(self, global: Self) -> Self {
    if self == Self::Default { global } else { self }
  }

  pub fn samples_per_cell(self) -> usize {
    if self == Self::Tty { 1 } else { 2 }
  }
}
