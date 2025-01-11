#[derive(Clone, Debug)]
#[repr(C)]
pub struct SystemdUnits {
    pub loaded_units: Vec<String>,
    pub enabled_units: Vec<String>,
}

impl SystemdUnits {
    pub fn new() -> Self {
        Self { loaded_units: Vec::new(), enabled_units: Vec::new() }
    }
}

impl Default for SystemdUnits {
    fn default() -> Self {
        Self::new()
    }
}
