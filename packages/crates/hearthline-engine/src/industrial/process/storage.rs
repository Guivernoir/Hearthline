use heapless::Vec as FixedList;
use hearthline_model::{PortId, Text};

use crate::runtime::collect_fixed;

pub(crate) type Ports = FixedList<PortId, 16>;
pub(crate) type TaggedValues<T> = FixedList<(Text<64>, T), 16>;

pub(crate) fn collect_ports(values: impl IntoIterator<Item = PortId>) -> Ports {
    collect_fixed(values)
}

pub(crate) fn tagged_values<T>(values: impl IntoIterator<Item = (Text<64>, T)>) -> TaggedValues<T> {
    collect_fixed(values)
}

pub(crate) fn get<'a, T>(values: &'a TaggedValues<T>, tag: &Text<64>) -> Option<&'a T> {
    values
        .iter()
        .find(|(candidate, _)| candidate == tag)
        .map(|(_, value)| value)
}

pub(crate) fn upsert<T>(values: &mut TaggedValues<T>, tag: Text<64>, value: T) {
    if let Some((_, current)) = values.iter_mut().find(|(candidate, _)| *candidate == tag) {
        *current = value;
    } else {
        assert!(
            values.push((tag, value)).is_ok(),
            "tagged runtime table exceeds capacity"
        );
    }
}
