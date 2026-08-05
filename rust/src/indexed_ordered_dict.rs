use std::collections::{HashMap, VecDeque};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};

use pyo3::exceptions::{PyIndexError, PyKeyError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyGenericAlias, PyIterator, PyList, PySlice, PyTuple, PyType};

/// Internal, pure-Rust ordered map.
///
/// This lets other Rust code work with an indexed ordered map without being
/// forced into PyO3/Python types. The PyO3 `IndexedOrderedDict` wrapper below
/// is just a specialization of this container.
#[derive(Clone)]
pub struct IndexedOrderedMap<K, V, S = RandomState> {
    pub map: HashMap<K, V, S>,
    pub order: VecDeque<K>,
}

impl<K, V, S> Default for IndexedOrderedMap<K, V, S>
where
    S: BuildHasher + Default,
{
    fn default() -> Self {
        Self {
            map: HashMap::with_hasher(S::default()),
            order: VecDeque::new(),
        }
    }
}

impl<K, V> IndexedOrderedMap<K, V, RandomState>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::with_hasher(RandomState::new()),
            order: VecDeque::new(),
        }
    }
}

impl<K, V, S> IndexedOrderedMap<K, V, S>
where
    K: Eq + Hash + Clone,
    S: BuildHasher,
{
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.map.insert(key.clone(), value).is_none() {
            self.order.push_back(key);
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    pub fn shift_remove(&mut self, key: &K) -> Option<V> {
        if let Some(val) = self.map.remove(key) {
            if let Some(pos) = self.order.iter().position(|x| x == key) {
                self.order.remove(pos);
            }
            Some(val)
        } else {
            None
        }
    }
    pub fn first(&self) -> Option<(&K, &V)> {
        let key = self.order.front()?;
        let val = self.map.get(key)?;
        Some((key, val))
    }
    pub fn last(&self) -> Option<(&K, &V)> {
        let key = self.order.back()?;
        let val = self.map.get(key)?;
        Some((key, val))
    }
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> {
        self.order.iter()
    }
    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> {
        self.order.iter().map(move |k| self.map.get(k).unwrap())
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> {
        self.order.iter().map(move |k| (k, self.map.get(k).unwrap()))
    }
    pub fn update(&mut self, other: &IndexedOrderedMap<K, V, S>)
    where
        V: Clone,
    {
        for k in &other.order {
            let v = other.map.get(k).unwrap();
            self.insert(k.clone(), v.clone());
        }
    }
}

/// A wrapper around Py<PyAny> to implement Hash and Eq
pub struct KeyWrapper(pub Py<PyAny>, pub isize);

impl Clone for KeyWrapper {
    fn clone(&self) -> Self {
        Python::attach(|py| {
            KeyWrapper(self.0.clone_ref(py), self.1)
        })
    }
}

impl KeyWrapper {
    pub fn new(key: Py<PyAny>) -> Self {
        let hash = Python::attach(|py| key.bind(py).hash().unwrap_or(0));
        KeyWrapper(key, hash)
    }

    fn clone_ref(&self, py: Python<'_>) -> Self {
        KeyWrapper(self.0.clone_ref(py), self.1)
    }
}
impl PartialEq for KeyWrapper {
    fn eq(&self, other: &Self) -> bool {
        if self.1 != other.1 {
            return false;
        }
        Python::attach(|py| {
            // check if self.0 == other.0 in Python
            self.0.bind(py).eq(other.0.bind(py)).unwrap_or(false)
        })
    }
}
impl Eq for KeyWrapper {}
impl Hash for KeyWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.1.hash(state);
    }
}

#[pyclass(module = "iod")]
pub struct IODValuesView {
    dict: Py<IndexedOrderedDict>,
}

#[pymethods]
impl IODValuesView {
    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.dict.borrow(py).map.len()
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyIterator>> {
        let dict = self.dict.borrow(py);
        let values: Vec<Py<PyAny>> = dict.map.values().map(|value| value.clone_ref(py)).collect();
        let list = PyList::new(py, &values)?;
        Ok(list.try_iter()?.unbind())
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dict = self.dict.borrow(py);

        if let Ok(slice) = key.cast::<PySlice>() {
            let indices = slice.indices(dict.map.len() as isize)?;
            let mut result: Vec<Py<PyAny>> = Vec::new();

            if indices.step > 0 {
                let mut i = indices.start;
                while i < indices.stop {
                    if let Some(value) = dict.iget(py, i) {
                        result.push(value);
                    }
                    i += indices.step;
                }
            } else {
                let mut i = indices.start;
                while i > indices.stop {
                    if let Some(value) = dict.iget(py, i) {
                        result.push(value);
                    }
                    i += indices.step;
                }
            }

            return Ok(PyList::new(py, &result)?.into_any().unbind());
        } else if let Ok(index) = key.extract::<isize>() {
            return dict.iget(py, index).ok_or_else(|| PyIndexError::new_err("index out of range"));
        }

        Err(PyTypeError::new_err("Invalid index type"))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let dict = self.dict.borrow(py);
        let values: Vec<String> = dict
            .map
            .values()
            .map(|v| v.bind(py).repr().unwrap().to_string())
            .collect();
        Ok(format!("IODValuesView([{}])", values.join(", ")))
    }
}

#[pyclass(module = "iod")]
pub struct IODKeysView {
    dict: Py<IndexedOrderedDict>,
}

#[pymethods]
impl IODKeysView {
    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.dict.borrow(py).map.len()
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyIterator>> {
        let dict = self.dict.borrow(py);
        let values: Vec<Py<PyAny>> = dict.map.keys().map(|key| key.0.clone_ref(py)).collect();
        let list = PyList::new(py, &values)?;
        Ok(list.try_iter()?.unbind())
    }

    fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dict = self.dict.borrow(py);

        if let Ok(slice) = key.cast::<PySlice>() {
            let indices = slice.indices(dict.map.len() as isize)?;
            let mut result: Vec<Py<PyAny>> = Vec::new();

            if indices.step > 0 {
                let mut i = indices.start;
                while i < indices.stop {
                    if let Some(value) = dict.ikey(py, i) {
                        result.push(value);
                    }
                    i += indices.step;
                }
            } else {
                let mut i = indices.start;
                while i > indices.stop {
                    if let Some(value) = dict.ikey(py, i) {
                        result.push(value);
                    }
                    i += indices.step;
                }
            }

            return Ok(PyList::new(py, &result)?.into_any().unbind());
        } else if let Ok(index) = key.extract::<isize>() {
            return dict.ikey(py, index).ok_or_else(|| PyIndexError::new_err("index out of range"));
        }

        Err(PyTypeError::new_err("Invalid index type"))
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let dict = self.dict.borrow(py);
        let keys: Vec<String> = dict
            .map
            .keys()
            .map(|k| k.0.bind(py).repr().unwrap().to_string())
            .collect();
        Ok(format!("IODKeysView([{}])", keys.join(", ")))
    }
}

#[pyclass(module = "iod")]
pub struct IODItemsView {
    dict: Py<IndexedOrderedDict>,
}

#[pymethods]
impl IODItemsView {
    pub fn __len__(&self, py: Python<'_>) -> usize {
        self.dict.borrow(py).map.len()
    }

    pub fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyIterator>> {
        let dict = self.dict.borrow(py);
        let items: Vec<(Py<PyAny>, Py<PyAny>)> = dict
            .map
            .iter()
            .map(|(k, v)| (k.0.clone_ref(py), v.clone_ref(py)))
            .collect();
        let list = PyList::new(py, &items)?;
        Ok(list.try_iter()?.unbind())
    }

    pub fn __getitem__(&self, py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let dict = self.dict.borrow(py);

        if let Ok(slice) = key.cast::<PySlice>() {
            let indices = slice.indices(dict.map.len() as isize)?;
            let mut result: Vec<(Py<PyAny>, Py<PyAny>)> = Vec::new();

            if indices.step > 0 {
                let mut i = indices.start;
                while i < indices.stop {
                    if let Some(item) = dict.iitem(py, i) {
                        result.push(item);
                    }
                    i += indices.step;
                }
            } else {
                let mut i = indices.start;
                while i > indices.stop {
                    if let Some(item) = dict.iitem(py, i) {
                        result.push(item);
                    }
                    i += indices.step;
                }
            }

            return Ok(PyList::new(py, &result)?.into_any().unbind());
        } else if let Ok(index) = key.extract::<isize>() {
            return dict
                .iitem(py, index)
                .map(|(key, value)| PyTuple::new(py, [key, value]).map(|tuple| tuple.into_any().unbind()))
                .unwrap_or_else(|| Err(PyIndexError::new_err("index out of range")));
        }

        Err(PyTypeError::new_err("Invalid index type"))
    }
}

#[pyclass(module = "iod", subclass)]
pub struct IndexedOrderedDict {
    pub map: IndexedOrderedMap<KeyWrapper, Py<PyAny>, RandomState>,
}

impl Default for IndexedOrderedDict {
    fn default() -> Self {
        Self {
            map: IndexedOrderedMap::new(),
        }
    }
}

#[pymethods]
impl IndexedOrderedDict {
    #[new]
    #[pyo3(signature = (*args, **kwargs))]
    fn __new__(
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut map = IndexedOrderedMap::<KeyWrapper, Py<PyAny>, RandomState>::new();

        if let Ok(arg) = args.get_item(0) {
            if let Ok(dict) = arg.cast::<PyDict>() {
                for (k, v) in dict.iter() {
                    map.insert(KeyWrapper::new(k.unbind()), v.unbind());
                }
            } else if let Ok(iter) = (&arg).try_iter() {
                 for item in iter {
                     let item = item?;
                     if let Ok(tuple) = item.cast::<PyTuple>() {
                         if tuple.len() == 2 {
                             let k = tuple.get_item(0)?.unbind();
                             let v = tuple.get_item(1)?.unbind();
                             map.insert(KeyWrapper::new(k), v);
                         }
                     } else if let Ok(list) = item.cast::<PyList>() {
                          if list.len() == 2 {
                             let k = list.get_item(0)?.unbind();
                             let v = list.get_item(1)?.unbind();
                             map.insert(KeyWrapper::new(k), v);
                         }
                     }
                 }
            }
        }

        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                map.insert(KeyWrapper::new(k.unbind()), v.unbind());
            }
        }
        Ok(IndexedOrderedDict { map })
    }

    fn __len__(&self) -> usize {
        self.map.len()
    }
    
    fn __getitem__(&self, py: Python<'_>, key: Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        // Look up using KeyWrapper
        match self.map.get(&KeyWrapper::new(key.clone().unbind())) {
            Some(val) => Ok(val.clone_ref(py)),
            None => Err(PyKeyError::new_err(key.unbind())),
        }
    }

    fn __setitem__(&mut self, key: Py<PyAny>, value: Py<PyAny>) {
        self.map.insert(KeyWrapper::new(key), value);
    }

    fn __delitem__(&mut self, py: Python<'_>, key: Py<PyAny>) -> PyResult<()> {
        match self.map.shift_remove(&KeyWrapper::new(key.clone_ref(py))) {
            Some(_) => Ok(()),
            None => Err(PyKeyError::new_err(key)),
        }
    }

    fn __contains__(&self, key: Py<PyAny>) -> bool {
        self.map.get(&KeyWrapper::new(key)).is_some()
    }   

    fn __iter__(&self) -> PyResult<Py<PyIterator>> {
        Python::attach(|py| {
            let keys: Vec<Py<PyAny>> = self.map.keys().map(|k| k.0.clone_ref(py)).collect();
            let list = PyList::new(py, &keys)?;
            let iter = list.try_iter()?;
            Ok(iter.unbind())
        })
    }
    fn __eq__(&self, other: Py<PyAny>) -> PyResult<bool> {
        self.compare_with(other, |a, b| a.eq(b), true)
    }
    fn __ne__(&self, other: Py<PyAny>) -> PyResult<bool> {
        self.compare_with(other, |a, b| a.ne(b), true).map(|eq| !eq)
    }
    fn __or__(&self, value: Py<PyAny>) -> PyResult<IndexedOrderedDict> {
        Python::attach(|py| {
            let mut new_dict = self.copy();
            if let Ok(other_dict) = value.extract::<PyRef<IndexedOrderedDict>>(py) {
                for (k, v) in other_dict.map.iter() {
                    new_dict.map.insert(k.clone_ref(py), v.clone_ref(py));
                }
                Ok(new_dict)
            } else if let Ok(other_dict) = value.bind(py).cast::<PyDict>() {
                for (k, v) in other_dict.iter() {
                    new_dict.map.insert(KeyWrapper::new(k.unbind()), v.unbind());
                }
                Ok(new_dict)
            } else {
                let type_name = value.bind(py).get_type();
                Err(PyTypeError::new_err(format!("unsupported operand type(s) for |: 'IndexedOrderedDict' and {}", type_name)))
            }
        })
    }
    fn __ior__(&mut self, m: &Bound<'_, PyDict>){
        for (k, v) in m.iter() {
            self.map.insert(KeyWrapper::new(k.unbind()), v.unbind());
        }
    }
    fn __reversed__(&self)-> PyResult<Py<PyIterator>> {
        Python::attach(|py| {
            let keys: Vec<Py<PyAny>> = self.map.keys().rev().map(|k| k.0.clone_ref(py)).collect();
            let list = PyList::new(py, &keys)?;
            let iter = list.try_iter()?;
            Ok(iter.unbind())
        })
    }
    fn update(&mut self, m: Py<PyAny>){
        Python::attach(|py| {
            if let Ok(other_dict) = m.extract::<PyRef<IndexedOrderedDict>>(py) {
                for (k, v) in other_dict.map.iter() {
                    self.map.insert(k.clone_ref(py), v.clone_ref(py));
                }
            } else if let Ok(other_dict) = m.bind(py).cast::<PyDict>() {
                for (k, v) in other_dict.iter() {
                    self.map.insert(KeyWrapper::new(k.unbind()), v.unbind());
                }
            }
        });
    }
    fn keys_list(slf: PyRef<Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        let keys: Vec<Py<PyAny>> = slf.map.keys().map(|k| k.0.clone_ref(py)).collect();
        PyList::new(py, &keys).map(|l| l.unbind())
    }

    fn values_list(slf: PyRef<Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        let values: Vec<Py<PyAny>> = slf.map.values().map(|v| v.clone_ref(py)).collect();
        PyList::new(py, &values).map(|l| l.unbind())
    }

    fn items_list(slf: PyRef<Self>) -> PyResult<Py<PyList>> {
        let py = slf.py();
        let items: Vec<(Py<PyAny>, Py<PyAny>)> = slf
            .map
            .iter()
            .map(|(k, v)| (k.0.clone_ref(py), v.clone_ref(py)))
            .collect();
        PyList::new(py, &items).map(|l| l.unbind())
    }

    fn keys(slf: Bound<'_, Self>) -> IODKeysView {
        IODKeysView { dict: slf.unbind() }
    }
    fn values(slf: Bound<'_, Self>) -> IODValuesView {
        IODValuesView { dict: slf.unbind() }
    }
    fn items(slf: Bound<'_, Self>) -> IODItemsView {
        IODItemsView { dict: slf.unbind() }
    }
    fn clear(&mut self) {
        self.map.clear();
    }
    
    fn copy(&self) -> Self {
        Python::attach(|py| {
            let mut new_map = IndexedOrderedMap::<KeyWrapper, Py<PyAny>, RandomState>::new();
            for (k, v) in self.map.iter() {
                new_map.insert(k.clone_ref(py), v.clone_ref(py));
            }
            IndexedOrderedDict { map: new_map }
        })
    }
    #[pyo3(signature = (key, default=None))]
    fn get(&self, py: Python<'_>, key: Py<PyAny>, default: Option<Py<PyAny>>) -> Option<Py<PyAny>> {
        match self.map.get(&KeyWrapper::new(key)) {
            Some(val) => Some(val.clone_ref(py)),
            None => default,
        }
    }

    #[pyo3(signature = (key=None, default=None))]
    fn pop(&mut self, py: Python<'_>, key: Option<Py<PyAny>>, default: Option<Py<PyAny>>) -> PyResult<Py<PyAny>> {
        match key{
            None => {
                let (k, v) = {
                    let (k, v) = self
                        .map
                        .last()
                        .ok_or_else(|| PyKeyError::new_err("dictionary is empty"))?;
                    (k.clone_ref(py), v.clone_ref(py))
                }; // <-- borrow from last() ends here
                self.map.shift_remove(&k);
                Ok(v)
            }
            Some(key) => {
                match self.map.shift_remove(&KeyWrapper::new(key.clone_ref(py))) {
                    Some(val) => Ok(val),
                    None => {
                        if let Some(d) = default {
                            Ok(d)
                        } else {
                            Err(PyKeyError::new_err(key))
                        }
                    }
                }
            }
        }
    }

    #[pyo3(signature = (key=None))]
    fn popitem(&mut self, py: Python<'_>, key: Option<Py<PyAny>>) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
        match key {
            Some(k) => {
                match self.map.shift_remove(&KeyWrapper::new(k.clone_ref(py))) {
                    Some(v) => Ok((k, v)),
                    None => Err(PyKeyError::new_err(k)),
                }
            }
            None => {
                if self.map.is_empty() {
                    return Err(PyKeyError::new_err("dictionary is empty"));
                }
                let k = self.map.order.pop_back().unwrap();
                let v = self.map.map.remove(&k).unwrap();
                Ok((k.0, v))
            }
        }
    }
     
    fn iget(&self, py: Python<'_>, index: isize) -> Option<Py<PyAny>> {
        // get the value at the given index with O(1) access
        // if index is negative, count from the end, if index>length, return None
        self.normalize_index(index)
            .and_then(|uindex| self.map.order.get(uindex).map(|k| self.map.map.get(k).unwrap().clone_ref(py)))
        
    }
    fn ikey(&self, py: Python<'_>, index: isize) -> Option<Py<PyAny>> {
        // get the key at the given index with O(1) access
        self.normalize_index(index)
            .and_then(|uindex| self.map.order.get(uindex).map(|k| k.0.clone_ref(py)))
    }
    fn iitem(&self, py: Python<'_>, index: isize) -> Option<(Py<PyAny>, Py<PyAny>)> {
        // get the (key, value) at the given index with O(1) access
        self.normalize_index(index)
            .and_then(|uindex| {
                self.map
                    .order
                    .get(uindex)
                    .map(|k| (k.0.clone_ref(py), self.map.map.get(k).unwrap().clone_ref(py)))
            })
    }
    #[pyo3(signature = (index=None))]
    fn ipop(&mut self, index: Option<isize>) -> PyResult<Py<PyAny>> {
        let idx = index.unwrap_or(-1);
        let uidx = self.normalize_index(idx)
            .ok_or_else(|| PyIndexError::new_err("index out of range"))?;
        let key = self.map.order.remove(uidx).unwrap();
        let val = self.map.map.remove(&key).unwrap();

        Ok(val)
    }
    #[pyo3(signature = (index=None))]
    fn ipopitem(&mut self, index: Option<isize>) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
        let idx = index.unwrap_or(-1);
        let uidx = self.normalize_index(idx)
            .ok_or_else(|| PyIndexError::new_err("index out of range"))?;
        let key = self.map.order.remove(uidx).unwrap();
        let val = self.map.map.remove(&key).unwrap();

        Ok((key.0, val))
    }

    #[pyo3(signature = (key, default=None))]
    fn setdefault(&mut self, py: Python<'_>, key: Py<PyAny>, default: Option<Py<PyAny>>) -> Py<PyAny> {
        if let Some(val) = self.map.get(&KeyWrapper::new(key.clone_ref(py))) {
            return val.clone_ref(py);
        }
        let val = default.unwrap_or_else(|| py.None());
        self.map.insert(KeyWrapper::new(key), val.clone_ref(py));
        val
    }

    #[pyo3(signature = (key, last=true))]
    fn move_to_end(&mut self, py: Python<'_>, key: Py<PyAny>, last: bool) -> PyResult<()> {
        // Move the specified key to the end (last=true) or beginning (last=false) of the ordered dictionary.
        if let Some(index) = self.map.order.iter().position(|x| x == &KeyWrapper::new(key.clone_ref(py))) {
            let k = self.map.order.remove(index).unwrap();
            if last { 
                self.map.order.push_back(k);
            } else {
                self.map.order.push_front(k);
            }
            Ok(())
        } else {
            Err(PyKeyError::new_err(key))
        }
    }

    #[pyo3(signature = (*, key=None, reverse=false))]
    fn sort(slf: &Bound<'_, Self>, py: Python<'_>, key: Option<Py<PyAny>>, reverse: bool) -> PyResult<()> {
        let keys: Vec<Py<PyAny>> = {
            let slf_ref = slf.borrow();
            slf_ref.map.keys().map(|k| k.0.clone_ref(py)).collect()
        };
        let py_keys = PyList::new(py, &keys)?;
        
        let kwargs = PyDict::new(py);
        if let Some(k) = key {
            kwargs.set_item("key", k)?;
        }
        kwargs.set_item("reverse", reverse)?;

        // Use Python's sort to use custom key functions
        py_keys.call_method("sort", (), Some(&kwargs))?; 
        
        let mut slf_mut = slf.borrow_mut();
        let mut new_order = VecDeque::with_capacity(slf_mut.map.len());
        for key_obj in py_keys.iter() {
            let key_wrapper = KeyWrapper::new(key_obj.unbind());
            if slf_mut.map.map.contains_key(&key_wrapper) {
                new_order.push_back(key_wrapper);
            }
        }        
        slf_mut.map.order = new_order;
        Ok(())
    }

    #[classmethod]
    #[pyo3(signature = (iterable, value=None))]
    fn fromkeys(_cls: &Bound<'_, PyType>, iterable: &Bound<'_, PyAny>, value: Option<Py<PyAny>>) -> PyResult<Self> {
        let py = iterable.py();
        let mut map = IndexedOrderedMap::<KeyWrapper, Py<PyAny>, RandomState>::new();
        for item in iterable.try_iter()? {
            let key = item?.unbind();
            let val = value.as_ref().map(|v| v.clone_ref(py)).unwrap_or_else(|| py.None());
            map.insert(KeyWrapper::new(key), val);
        }
        Ok(IndexedOrderedDict { map })
    }
    fn __getstate__(&self) -> PyResult<Py<PyTuple>> {
        Python::attach(|py| {
            let items: Vec<(Py<PyAny>, Py<PyAny>)> = self
                .map
                .iter()
                .map(|(k, v)| (k.0.clone_ref(py), v.clone_ref(py)))
                .collect();
            PyTuple::new(py, &items).map(|t| t.unbind())
        })
    }
    fn __setstate__(&mut self, state: &Bound<'_, PyAny>) -> PyResult<()> {
        let items = state.cast::<PyTuple>()?;
        self.map.clear();
        for item in items.iter() {
            let tuple = item.cast::<PyTuple>()?;
            if tuple.len() != 2 {
                return Err(PyTypeError::new_err("invalid state"));
            }
            let k = tuple.get_item(0)?.unbind();
            let v = tuple.get_item(1)?.unbind();
            self.map.insert(KeyWrapper::new(k), v);
        }
        Ok(())
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        Ok(PyGenericAlias::new(cls.py(), cls.as_any(), key)?.into_any().unbind())
    }

    fn index_of(&self, py: Python<'_>, key: Py<PyAny>) -> PyResult<usize> {
        match self.map.order.iter().position(|x| x == &KeyWrapper::new(key.clone_ref(py))) {
            Some(i) => Ok(i),
            None => Err(PyValueError::new_err(format!("{:?} is not in list", key))),
        }
    }
        
    fn __repr__(&self) -> PyResult<String> {
        let mut items = Vec::new();
        for (k, v) in self.map.iter() {
            let k_repr = Python::attach(|py| k.0.bind(py).repr().unwrap().to_string());
            let v_repr = Python::attach(|py| v.bind(py).repr().unwrap().to_string());
            items.push(format!("{}: {}", k_repr, v_repr));
        }
        Ok(format!("IndexedOrderedDict({{{}}})", items.join(", ")))
    }
}

impl IndexedOrderedDict {
    fn normalize_index(&self, index: isize) -> Option<usize> {
        let len = isize::try_from(self.map.len()).ok()?;
        let idx = if index < 0 {
            len.checked_add(index as isize)?
        } else {
            index as isize
        };

        if (0..len).contains(&idx) {
            Some(idx as usize)
        } else {
            None
        }
    }
    
    // --- Rust-friendly helpers for other PyO3 code ---
    // These are intended for internal Rust use (your other `#[pyclass]` impls).
    // They let you work with *typed* keys/values at the boundary and keep the
    // storage as `PyObject`.    
    pub fn insert_py(&mut self, _py: Python<'_>, key: &Bound<'_, PyAny>, value: &Bound<'_, PyAny>) {
        self.map.insert(KeyWrapper::new(key.clone().unbind()), value.clone().unbind());
    }

    pub fn get_as<'a, 'py: 'a, T>(&'a self, py: Python<'py>, key: &Bound<'py, PyAny>) -> Option<T>
    where
        T: FromPyObject<'a, 'py>,
    {
        match self.map.get(&KeyWrapper::new(key.clone().unbind())) {
            Some(val) => val.bind(py).extract().ok(),
            None => None,
        }
    }

    pub fn get_value(&self, py: Python<'_>, key: Py<PyAny>) -> Option<Py<PyAny>> {
        self.map.get(&KeyWrapper::new(key)).map(|v| v.clone_ref(py))
    }

    pub fn insert_item(&mut self, key: Py<PyAny>, value: Py<PyAny>) {
        self.map.insert(KeyWrapper::new(key), value);
    }

    pub fn first(&self) -> Option<(&Py<PyAny>, &Py<PyAny>)> {
        self.map.first().map(|(k, v)| (&k.0, v))
    }

    pub fn last(&self) -> Option<(&Py<PyAny>, &Py<PyAny>)> {
        self.map.last().map(|(k, v)| (&k.0, v))
    }

    fn compare_with<F>(&self, other: Py<PyAny>, op: F, check_len: bool) -> PyResult<bool>
    where
        F: Fn(&Bound<'_, PyAny>, &Bound<'_, PyAny>) -> PyResult<bool>,
    {
        Python::attach(|py| {
            if let Ok(other_dict) = other.extract::<PyRef<IndexedOrderedDict>>(py) {
                if check_len && self.map.len() != other_dict.map.len() {
                    return Ok(false);
                }
                for (k, v) in self.map.iter() {
                    match other_dict.map.get(k) {
                        Some(ov)=>{
                            let v_bound: Bound<'_, PyAny> = v.bind(py).clone();
                            let ov_bound: Bound<'_, PyAny> = ov.bind(py).clone();
                            if !op(&v_bound, &ov_bound)? {
                                return Ok(false);
                            }
                        }
                        None => return Ok(false),
                    }
                }
                Ok(true)
            } else {
                Ok(false)
            }
        })
    }
}