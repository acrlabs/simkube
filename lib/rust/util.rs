use std::cmp::{
    min,
    Ord,
};

pub fn min_some<T: Ord>(o1: Option<T>, o2: Option<T>) -> Option<T> {
    if o1.is_none() {
        o2
    } else if o2.is_none() {
        o1
    } else {
        min(o1, o2)
    }
}

#[cfg(test)]
mod test {
    use rstest::*;

    use super::*;

    #[rstest]
    #[case::both_none(None, None, None)]
    #[case::left_some(Some(1), None, Some(1))]
    #[case::right_some(None, Some(1), Some(1))]
    #[case::both_some(Some(2), Some(1), Some(1))]
    fn test_min_some(#[case] o1: Option<i32>, #[case] o2: Option<i32>, #[case] expected: Option<i32>) {
        assert_eq!(min_some(o1, o2), expected);
    }
}
