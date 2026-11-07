use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Sym(u32);

impl Sym {
    pub const NONE: Sym = Sym(0);

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn is_none(self) -> bool {
        self == Sym::NONE
    }
}

#[derive(Debug, Clone)]
pub struct Interner {
    strings: Vec<Box<str>>,
    lookup: HashMap<Box<str>, Sym>,
}

impl Default for Interner {
    fn default() -> Self {
        Self {
            strings: vec!["".into()],
            lookup: HashMap::new(),
        }
    }
}

impl Interner {
    pub fn intern(&mut self, s: &str) -> Sym {
        if s.is_empty() {
            return Sym::NONE;
        }
        if let Some(&sym) = self.lookup.get(s) {
            return sym;
        }
        let sym = Sym(u32::try_from(self.strings.len()).expect("fewer than 2^32 strings"));
        self.strings.push(s.into());
        self.lookup.insert(s.into(), sym);
        sym
    }

    pub fn get(&self, sym: Sym) -> &str {
        &self.strings[sym.index()]
    }
}
