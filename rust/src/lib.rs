use pyo3::prelude::*;

mod indexed_ordered_dict;
use indexed_ordered_dict::IndexedOrderedDict;

/// A Python module implemented in Rust.
#[pymodule]
// #[pyo3(name = "__init__")]
fn iod(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<IndexedOrderedDict>()?;
    Ok(())
}
