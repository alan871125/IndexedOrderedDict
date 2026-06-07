# IndexedOrderedDictionary

This is an implementation of an Indexed Ordered Dictionary for Python with rust

## Complexity

The implementation is backed by Rust's `indexmap::IndexMap`, so lookup and
insertion are hash-table based while insertion order is preserved.

| Operation | Average complexity | Notes |
| --- | --- | --- |
| `len(d)` | `O(1)` | Stored length |
| `d[key]`, `d.get(key)`, `key in d` | `O(1)` | Worst case can degrade with hash collisions |
| `d[key] = value` | `O(1)` | Existing keys keep their current position |
| `del d[key]`, `pop(key)` | `O(n)` | Preserves order by shifting later entries |
| `popitem(last=True)` | `O(1)` | Removes the last entry |
| `popitem(last=False)` | `O(n)` | Removes the first entry and shifts later entries |
| `keys()`, `values()`, `items()`, iteration | `O(n)` | Builds Python list-backed views/iterators |
| `copy()` | `O(n)` | Clones Python object references |
| `update(other)` | `O(m)` | `m` is the number of incoming items |
| `d1 \| d2` | `O(n + m)` | Copies the left dictionary, then inserts the right |
| `move_to_end(key)` | `O(n)` | Moving an entry shifts surrounding entries |
| `index_of(key)` | `O(1)` | Average hash lookup by key |
| `sort()` | `O(n log n)` | Python sorting dominates |
| Equality comparison | `O(n)` | Average case; compares each key/value pair |

Python key hashing and equality are still used, so custom keys with expensive
`__hash__` or `__eq__` methods can make individual operations slower.

