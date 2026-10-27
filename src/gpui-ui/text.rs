//! Grapheme, bidi-isolate, search-fold and display helpers.
//!
//! Strings stay logical UTF-8. GPUI shapes; this module only isolates LTR
//! runs, folds Persian ye/ke for search, and truncates with the ellipsis on
//! the visual end.

use unicode_segmentation::UnicodeSegmentation;

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

pub(crate) fn fa_ye_ke(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ي' | 'ى' => 'ی',
            'ك' => 'ک',
            c => c,
        })
        .collect()
}

pub(crate) fn fold_search(s: &str) -> String {
    fa_ye_ke(s).to_lowercase()
}

pub(crate) fn delete_last_grapheme(s: &str) -> &str {
    match s.grapheme_indices(true).next_back() {
        Some((i, _)) => &s[..i],
        None => "",
    }
}

pub(crate) fn grapheme_count(s: &str) -> usize {
    s.graphemes(true).count()
}

pub(crate) fn fa_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32('۰' as u32 + u32::from(c) - u32::from('0')).unwrap_or(c),
            c => c,
        })
        .collect()
}

pub(crate) fn from_fa_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '۰'..='۹' => char::from_u32(u32::from('0') + u32::from(c) - u32::from('۰')).unwrap_or(c),
            '٠'..='٩' => char::from_u32(u32::from('0') + u32::from(c) - u32::from('٠')).unwrap_or(c),
            c => c,
        })
        .collect()
}

pub(crate) fn ui_text(s: &str) -> String {
    if crate::i18n::is_fa() {
        fa_digits(&fa_ye_ke(s))
    } else {
        s.to_string()
    }
}

pub(crate) fn fa_input(s: &str) -> String {
    if crate::i18n::is_fa() {
        fa_digits(s)
    } else {
        s.to_string()
    }
}

pub(crate) fn ltr_isolate(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let inner = if crate::i18n::is_fa() {
        fa_digits(s)
    } else {
        s.to_string()
    };
    if crate::rtl::on() {
        format!("{FSI}{inner}{PDI}")
    } else {
        inner
    }
}

pub(crate) fn ellipsize(s: &str, max_graphemes: usize) -> String {
    if max_graphemes == 0 {
        return String::new();
    }
    let count = grapheme_count(s);
    if count <= max_graphemes {
        return s.to_string();
    }
    let keep = max_graphemes.saturating_sub(1);
    let mut end = s.len();
    for (i, (byte, _)) in s.grapheme_indices(true).enumerate() {
        if i == keep {
            end = byte;
            break;
        }
    }
    format!("{}…", &s[..end])
}

pub(crate) fn display_fa_ui(s: &str) -> String {
    ui_text(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backspace_deletes_one_grapheme() {
        assert_eq!(delete_last_grapheme("گزارش"), "گزار");
        assert_eq!(delete_last_grapheme("á"), "");
        assert_eq!(delete_last_grapheme(""), "");
        assert_eq!(delete_last_grapheme("سلام دنیا"), "سلام دنی");
    }

    #[test]
    fn ye_ke_search_matches_variants() {
        let name = fold_search("گزارش يك.zip");
        assert!(name.contains(&fold_search("گزارش یک")));
        assert!(fold_search("یک").contains(&fold_search("يك")));
        assert_eq!(fold_search("يك"), fold_search("یک"));
        assert_eq!(fold_search("ك"), fold_search("ک"));
    }

    #[test]
    fn ellipsize_keeps_logical_start() {
        let s = ellipsize("گزارش-۱۴۰۳-خیلی-طولانی.zip", 8);
        assert!(s.ends_with('…'));
        assert!(s.starts_with("گزارش"));
        assert_eq!(grapheme_count(&s), 8);
    }

    #[test]
    fn fa_display_maps_arabic_ye_ke() {
        assert_eq!(fa_ye_ke("يك"), "یک");
        assert_eq!(fa_ye_ke("گزارش"), "گزارش");
    }

    #[test]
    fn fa_digits_maps_ascii() {
        assert_eq!(fa_digits("12.4 MB"), "۱۲.۴ MB");
        assert_eq!(fa_digits("لیست 1"), "لیست ۱");
        assert_eq!(from_fa_digits("لیست ۱"), "لیست 1");
        assert_eq!(from_fa_digits("٤"), "4");
    }

    #[test]
    fn display_helpers_do_not_reverse_logical_text() {
        for s in [
            "سلام.",
            "سلام 123",
            "نسخه 2.5 برنامه.",
            "فایل test-2026.zip دانلود شد.",
            "خطا: فایل پیدا نشد (404).",
            "دانلود 25% تکمیل شد.",
        ] {
            let out = fa_digits(s);
            assert!(!out.starts_with('.'), "{s:?} -> {out:?}");
            assert_eq!(s.chars().next(), out.chars().next());
            assert_eq!(s.chars().count(), out.chars().count());
        }
    }

}
