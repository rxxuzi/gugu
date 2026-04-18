use crate::PortId;

/// Undirected connection between two ports.
/// Normalized so that the smaller PortId comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bond(PortId, PortId);

impl Bond {
    pub fn new(a: PortId, b: PortId) -> Self {
        if a.raw() <= b.raw() {
            Self(a, b)
        } else {
            Self(b, a)
        }
    }

    pub fn ports(self) -> (PortId, PortId) {
        (self.0, self.1)
    }

    /// Given one end, return the other. Panics if `p` is neither end.
    pub fn other(self, p: PortId) -> PortId {
        if p == self.0 {
            self.1
        } else if p == self.1 {
            self.0
        } else {
            panic!("{p} is not an endpoint of bond ({}, {})", self.0, self.1);
        }
    }

    pub fn has(self, p: PortId) -> bool {
        p == self.0 || p == self.1
    }
}

impl std::fmt::Display for Bond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -- {}", self.0, self.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_order() {
        let a = PortId::new(5);
        let b = PortId::new(2);
        let w1 = Bond::new(a, b);
        let w2 = Bond::new(b, a);
        assert_eq!(w1, w2);
        assert_eq!(w1.ports(), (b, a));
    }

    #[test]
    fn other_endpoint() {
        let a = PortId::new(1);
        let b = PortId::new(3);
        let w = Bond::new(a, b);
        assert_eq!(w.other(a), b);
        assert_eq!(w.other(b), a);
    }

    #[test]
    #[should_panic]
    fn other_panics_on_bad_port() {
        let w = Bond::new(PortId::new(1), PortId::new(2));
        w.other(PortId::new(99));
    }
}
