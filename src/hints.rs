use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Child {
    pub absolute_x: i32,
    pub absolute_y: i32,
    pub width: i32,
    pub height: i32,
}

pub type HintMap = HashMap<String, Child>;

/// Generate hint labels for a set of children using the provided alphabet.
pub fn generate_hints(children: &[Child], alphabet: &str) -> HintMap {
    let mut result = HintMap::new();
    if children.is_empty() || alphabet.is_empty() {
        return result;
    }

    let base: Vec<char> = alphabet.chars().collect();
    let radix = base.len() as u32;
    let needed = (children.len() as f64).log(radix as f64).ceil() as u32;

    for (idx, child) in children.iter().enumerate() {
        let mut n = idx as u32;
        let mut label_chars = Vec::new();
        for _ in 0..needed {
            let digit = n % radix;
            label_chars.push(base[digit as usize]);
            n /= radix;
        }
        if n > 0 {
            label_chars.push(base[(n % radix) as usize]);
        }
        label_chars.reverse();
        let label: String = label_chars.into_iter().collect();
        result.insert(label, *child);
    }

    result
}
