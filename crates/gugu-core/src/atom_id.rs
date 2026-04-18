#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtomId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(u32);

impl AtomId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl PortId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for AtomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "n{}", self.0)
    }
}

impl std::fmt::Display for PortId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "p{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_distinct() {
        let a = AtomId::new(0);
        let b = AtomId::new(1);
        assert_ne!(a, b);
        assert_eq!(a, AtomId::new(0));
    }

    #[test]
    fn display() {
        assert_eq!(AtomId::new(42).to_string(), "n42");
        assert_eq!(PortId::new(7).to_string(), "p7");
    }
}
