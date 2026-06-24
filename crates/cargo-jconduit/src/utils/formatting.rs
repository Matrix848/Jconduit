pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut cap_next = false;

    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            cap_next = true;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else if cap_next {
            result.push(c.to_ascii_uppercase());
            cap_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn to_pascal_case(s: &str) -> String {
    capitalize(&to_camel_case(s))
}

pub fn screaming_snake_to_pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|word| !word.is_empty()) // Handles accidental double underscores "__"
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => {
                    // Capitalize the first letter, lowercase the remaining letters
                    f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

pub struct Writer {
    buf: String,
    level: usize,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            level: 0,
        }
    }

    pub fn line(&mut self, s: &str) {
        if !self.buf.is_empty() {
            self.buf.push('\n');
        }
        self.buf.push_str(&"    ".repeat(self.level));
        self.buf.push_str(s);
    }

    pub fn indent(&mut self) {
        self.level += 1;
    }
    pub fn dedent(&mut self) {
        self.level = self.level.saturating_sub(1);
    }
    pub fn finish(self) -> String {
        self.buf
    }
}
