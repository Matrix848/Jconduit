use bindgen::callbacks::{ItemInfo, ItemKind, ParseCallbacks};

#[derive(Debug)]
pub(crate) struct PrefixStripper {
    prefix: String,
}

impl PrefixStripper {
    pub fn new(prefix: String) -> Self {
        Self { prefix }
    }
}

impl ParseCallbacks for PrefixStripper {
    fn item_name(&self, _item_info: ItemInfo) -> Option<String> {
        if (matches!(_item_info.kind, ItemKind::Function)
            || matches!(_item_info.kind, ItemKind::Type))
            && let Some(stripped_name) = _item_info.name.strip_prefix(&self.prefix)
        {
            return Some(stripped_name.to_string());
        }
        None
    }
}
