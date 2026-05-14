use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct SymbolTable {
    labels: HashMap<String, u32>,
    equs: HashMap<String, i32>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn define(&mut self, name: &str, addr: u32) {
        self.labels.insert(name.to_owned(), addr);
    }

    pub fn define_equ(&mut self, name: &str, val: i32) {
        self.equs.insert(name.to_owned(), val);
    }

    pub fn resolve(&self, name: &str) -> Option<u32> {
        self.labels.get(name).copied()
    }

    pub fn resolve_equ(&self, name: &str) -> Option<i32> {
        self.equs.get(name).copied()
    }

    #[allow(dead_code)]
    pub fn all(&self) -> &HashMap<String, u32> {
        &self.labels
    }
}
