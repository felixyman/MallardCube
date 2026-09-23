//! Coordinate lookups for N-dimension results (plan 051).
//!
//! Both renderers ask the same question — "what is the value at this
//! coordinate?" — where a coordinate is one slot per group-by dimension and a
//! `None` slot means the `(All)` member, i.e. "sum over that dimension".
//!
//! The first implementation scanned every result row per cell
//! (`rows.iter().find(..)`), which is O(cells x rows): an 84k-cell cross-tab
//! spent 12.7 s rendering and a three-field layout never finished. Here the
//! exact coordinates resolve through one map, and each wildcard *mask* the
//! caller actually uses gets one roll-up pass, so the cell loop is O(cells).

use std::collections::{HashMap, HashSet};

/// The wildcard mask of a coordinate: bit *i* set means slot *i* is `(All)`.
pub fn coord_mask(coord: &[Option<&str>]) -> u32 {
    coord
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.is_none())
        .fold(0u32, |mask, (i, _)| mask | (1 << i))
}

/// The lookup key of the slots a mask leaves specified (mask 0 = all slots).
fn coord_key(coord: &[Option<&str>], mask: u32) -> String {
    let mut key = String::new();
    for (i, slot) in coord.iter().enumerate() {
        if mask & (1 << i) != 0 {
            continue;
        }
        if !key.is_empty() {
            key.push('\u{1}');
        }
        key.push_str(slot.unwrap_or_default());
    }
    key
}

/// Value lookup for a coordinate, exact or `(All)`-rolled.
pub struct CoordIndex<'a> {
    exact: HashMap<String, &'a [f64]>,
    /// mask -> surviving-key -> summed values, built only for the masks the
    /// caller's coordinates use.
    rollups: HashMap<u32, HashMap<String, Vec<f64>>>,
    n_measures: usize,
}

impl<'a> CoordIndex<'a> {
    /// Build the exact map plus one roll-up per mask in `needed_masks`.
    pub fn build(
        rows: &'a [(Vec<String>, Vec<f64>)],
        n_measures: usize,
        needed_masks: &HashSet<u32>,
    ) -> Self {
        let mut exact: HashMap<String, &'a [f64]> = HashMap::with_capacity(rows.len());
        for (keys, values) in rows {
            exact.insert(keys.join("\u{1}"), values.as_slice());
        }
        let mut rollups: HashMap<u32, HashMap<String, Vec<f64>>> = HashMap::new();
        for mask in needed_masks.iter().copied().filter(|m| *m != 0) {
            let mut buckets: HashMap<String, Vec<f64>> = HashMap::new();
            for (keys, values) in rows {
                // The mask indexes coordinate slots; the data keys are in the
                // same order (the plan's group-by columns).
                let key = keys
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) == 0)
                    .map(|(_, k)| k.as_str())
                    .collect::<Vec<_>>()
                    .join("\u{1}");
                let bucket = buckets.entry(key).or_insert_with(|| vec![0.0; n_measures]);
                for (mi, value) in values.iter().enumerate().take(n_measures) {
                    bucket[mi] += *value;
                }
            }
            rollups.insert(mask, buckets);
        }
        Self {
            exact,
            rollups,
            n_measures,
        }
    }

    /// The value of measure `mi` at `coord`, or `None` when no row matches —
    /// the reference omits those cells rather than sending zeros.
    pub fn get(&self, coord: &[Option<&str>], mi: usize) -> Option<f64> {
        if mi >= self.n_measures {
            return None;
        }
        let mask = coord_mask(coord);
        if mask == 0 {
            return self
                .exact
                .get(&coord_key(coord, 0))
                .map(|values| values.get(mi).copied().unwrap_or(0.0));
        }
        let key = coord_key(coord, mask);
        self.rollups
            .get(&mask)
            .and_then(|buckets| buckets.get(&key))
            .map(|values| values.get(mi).copied().unwrap_or(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<(Vec<String>, Vec<f64>)> {
        vec![
            (vec!["2024".into(), "Food".into()], vec![10.0]),
            (vec!["2024".into(), "Garden".into()], vec![5.0]),
            (vec!["2025".into(), "Food".into()], vec![7.0]),
        ]
    }

    #[test]
    fn exact_coordinates_resolve_and_misses_are_none() {
        let data = rows();
        let index = CoordIndex::build(&data, 1, &HashSet::new());
        assert_eq!(index.get(&[Some("2024"), Some("Food")], 0), Some(10.0));
        assert_eq!(
            index.get(&[Some("2026"), Some("Food")], 0),
            None,
            "an absent combination is omitted, not zero"
        );
        assert_eq!(index.get(&[Some("2024"), Some("Food")], 3), None);
    }

    #[test]
    fn all_slots_roll_up_over_their_dimension() {
        let data = rows();
        let masks: HashSet<u32> = [0b01, 0b10, 0b11].into_iter().collect();
        let index = CoordIndex::build(&data, 1, &masks);
        // (All) year, Food: 10 + 7
        assert_eq!(index.get(&[None, Some("Food")], 0), Some(17.0));
        // 2024, (All) category: 10 + 5
        assert_eq!(index.get(&[Some("2024"), None], 0), Some(15.0));
        // (All), (All): everything
        assert_eq!(index.get(&[None, None], 0), Some(22.0));
        // An (All) combination with no rows at all stays absent.
        assert_eq!(index.get(&[None, Some("Books")], 0), None);
    }

    #[test]
    fn a_mask_the_caller_did_not_ask_for_is_not_built() {
        let data = rows();
        let index = CoordIndex::build(&data, 1, &HashSet::new());
        assert_eq!(
            index.get(&[None, Some("Food")], 0),
            None,
            "no roll-up was requested for that mask"
        );
    }
}
