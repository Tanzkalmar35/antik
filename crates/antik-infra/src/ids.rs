use antik_app::ports::IdGen;

/// ULID identifiers: sortable, URL-safe and stable across a rename.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemIds;

impl IdGen for SystemIds {
    fn next(&self) -> String {
        ulid::Ulid::generate().to_string()
    }
}
