pub(crate) fn to_camel_case(s: &str) -> String {
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

pub(crate) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub(crate) struct Writer {
    buf: String,
    level: usize,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::new(),
            level: 0,
        }
    }

    pub(crate) fn line(&mut self, s: &str) {
        if !self.buf.is_empty() {
            self.buf.push('\n');
        }
        self.buf.push_str(&"    ".repeat(self.level));
        self.buf.push_str(s);
    }

    pub(crate) fn indent(&mut self) {
        self.level += 1;
    }
    pub(crate) fn dedent(&mut self) {
        self.level = self.level.saturating_sub(1);
    }
    pub(crate) fn finish(self) -> String {
        self.buf
    }
}
