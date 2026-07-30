use core::fmt::{self, Write as _};
use heapless::Vec as FixedList;
use hearthline_model::Text;

pub(crate) fn collect_fixed<T, I, const N: usize>(values: I) -> FixedList<T, N>
where
    I: IntoIterator<Item = T>,
{
    let mut output = FixedList::new();
    for value in values {
        assert!(
            output.push(value).is_ok(),
            "runtime collection exceeds fixed capacity"
        );
    }
    output
}

pub(crate) fn runtime_text<const N: usize>(arguments: fmt::Arguments<'_>) -> Text<N> {
    let mut output = Text::default();
    output
        .write_fmt(arguments)
        .expect("runtime message exceeds fixed capacity");
    output
}
