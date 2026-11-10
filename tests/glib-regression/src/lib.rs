#[cfg(test)]
mod tests {
    use glib::prelude::ToVariant;

    #[test]
    fn string_iteration_preserves_utf8_in_both_directions() {
        let value = ["first", "café", "last"].to_variant();
        assert_eq!(
            value.array_iter_str().unwrap().collect::<Vec<_>>(),
            ["first", "café", "last"]
        );
        assert_eq!(value.array_iter_str().unwrap().nth(1), Some("café"));
        assert_eq!(value.array_iter_str().unwrap().last(), Some("last"));
        assert_eq!(value.array_iter_str().unwrap().next_back(), Some("last"));
        assert_eq!(value.array_iter_str().unwrap().nth_back(1), Some("café"));

        let mut mixed = value.array_iter_str().unwrap();
        assert_eq!(mixed.next(), Some("first"));
        assert_eq!(mixed.next_back(), Some("last"));
        assert_eq!(mixed.next(), Some("café"));
        assert_eq!(mixed.next_back(), None);
        assert_eq!(mixed.next(), None);
    }

    #[test]
    fn empty_string_iteration_is_exhausted() {
        let value = Vec::<String>::new().to_variant();
        let mut iter = value.array_iter_str().unwrap();
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
        assert_eq!(iter.nth(0), None);
        assert_eq!(iter.nth_back(0), None);
        assert_eq!(iter.last(), None);
    }
}
