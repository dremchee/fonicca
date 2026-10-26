//! Case-insensitive filename ordering with numeric runs of any length.
use std::cmp::Ordering;

pub fn compare(a: &str, b: &str) -> Ordering {
    let (mut i, mut j) = (0, 0);
    let mut zeros = Ordering::Equal;
    while i < a.len() && j < b.len() {
        let ca = a[i..].chars().next().unwrap();
        let cb = b[j..].chars().next().unwrap();
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let (start_a, start_b) = (i, j);
            while i < a.len() && a.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b.as_bytes()[j].is_ascii_digit() {
                j += 1;
            }
            let na = a[start_a..i].trim_start_matches('0');
            let nb = b[start_b..j].trim_start_matches('0');
            let order = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if order != Ordering::Equal {
                return order;
            }
            if zeros == Ordering::Equal {
                zeros = (i - start_a).cmp(&(j - start_b));
            }
        } else {
            let order = ca.to_lowercase().cmp(cb.to_lowercase());
            if order != Ordering::Equal {
                return order;
            }
            i += ca.len_utf8();
            j += cb.len_utf8();
        }
    }
    (a.len() - i).cmp(&(b.len() - j)).then(zeros)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_use_numeric_runs_unicode_case_and_stable_leading_zeros() {
        let mut names = [
            "Трек 10",
            "трек 2",
            "Трек 01",
            "трек 1",
            "Трек 000",
            "Трек 0",
        ];
        names.sort_by(|a, b| compare(a, b));
        assert_eq!(
            names,
            [
                "Трек 0",
                "Трек 000",
                "трек 1",
                "Трек 01",
                "трек 2",
                "Трек 10"
            ]
        );
        assert_eq!(
            compare("Kick 2 take 10", "kick 2 take 3"),
            Ordering::Greater
        );
        assert_eq!(
            compare(
                "file 999999999999999999999999",
                "File 1000000000000000000000000"
            ),
            Ordering::Less
        );
        assert_eq!(compare("ÉCHO", "écho"), Ordering::Equal);
        assert_eq!(compare("a02a", "a2b"), Ordering::Less);
    }
}
