use pyo3::prelude::*;

mod indexed_ordered_dict;
use indexed_ordered_dict::{IndexedOrderedDict, IODKeysView, IODValuesView};

/// A Python module implemented in Rust.
#[pymodule]
// #[pyo3(name = "__init__")]
fn iod(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<IndexedOrderedDict>()?;
    m.add_class::<IODKeysView>()?;
    m.add_class::<IODValuesView>()?;
    Ok(())
}
