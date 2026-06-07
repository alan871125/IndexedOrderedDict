#!/usr/bin/env python

import pickle
from iod import IndexedOrderedDict
import pytest
class TestIndexedOrderedDict():
    def test_init(self):
        d = IndexedOrderedDict(
            {'a':1, 'b':2, 'c':3}
        )
        assert(len(d) == 3)
        assert(d['a'] == 1)
        assert(d['b'] == 2)
        assert(d['c'] == 3)

        d2 = IndexedOrderedDict(
            [('a',1), ('b',2), ('c',3)]
        )
        assert(len(d2) == 3)
        assert(d2['a'] == 1)
        assert(d2['b'] == 2)
        assert(d2['c'] == 3)

        d3 = IndexedOrderedDict(
            a=1,
            b=2,
            c=3
        )
        assert(len(d3) == 3)
        assert(d3['a'] == 1)
        assert(d3['b'] == 2)
        assert(d3['c'] == 3)
    def test_del(self):
        d = IndexedOrderedDict(
            a=1,
            b=[2],
            c="3"
        )

        del d["a"]
        assert("a" not in d)
        assert(d.keys().index('b')==0)
        assert(d.keys().index('c')==1)

    def test_iter(self):
        d = IndexedOrderedDict(
            {'a':1, 'b':2, 'c':3}
        )
        it = d.__iter__()
        assert(next(it) == 'a')
        assert(next(it) == 'b')
        assert(next(it) == 'c')
        
    def test_slicing(self):
        d = IndexedOrderedDict(
            {'b':1,'c':2,'a':3,'d':4 }
        )
        assert list(d.keys()[1:3]) == ['c', 'a']
        assert list(d.values()[:3]) == [1, 2, 3]
        assert list(d.items()[5:]) == []

    def test_reversed(self):
        d = IndexedOrderedDict(
            {'a':1, 'b':2, 'c':3}
        )
        it = d.__reversed__()
        assert(next(it) == 'c')
        assert(next(it) == 'b')
        assert(next(it) == 'a')
        
    def test_sort(self):
        d = IndexedOrderedDict(
            {'b':1,'c':2,'a':3, }
        )
        assert list(d.keys()) == ['b', 'c', 'a']
        assert list(d.values()) == [1, 2, 3]
        d.sort()
        assert list(d.keys()) == ['a', 'b', 'c']
        assert list(d.values()) == [3, 1, 2]
        d.sort(reverse=True)
        assert list(d.keys()) == ['c', 'b', 'a']
        assert list(d.values()) == [2, 1, 3]
        d.sort(key=lambda k: d[k])
        assert list(d.keys()) == ['b', 'c', 'a']
        assert list(d.values()) == [1, 2, 3]

    def test_clear(self):
        d = IndexedOrderedDict(foo='bar')
        assert len(d) == 1
        assert len(d.keys()) == 1
        assert len(d.values()) == 1
        d.clear()
        assert len(d) == 0
        assert len(d.keys()) == 0
        assert len(d.values()) == 0

    def test_pop(self):
        d = IndexedOrderedDict()
        d["foo"] = "bar"
        assert "foo" in d
        assert d.pop("foo")=="bar"
        assert "foo" not in d
        
    def test_popitem(self):
        d = IndexedOrderedDict({
            'a': 1,
            'b': 2,
            'c': 3,            
        })
        key, value = d.popitem()
        assert (key,value) ==('c',3)

    


    def test_setdefault(self):
        d = IndexedOrderedDict()
        d["a"] = "alpha"

        assert d.setdefault("a", None), "alpha"
        assert d.setdefault("b", "beta"), "beta"
        assert d.setdefault("b", "gamma"), "beta"

    
    def test_eq(self):
        a = IndexedOrderedDict(a=1, b=2, c=3)
        b = IndexedOrderedDict(a=1, b=2, c=3)
        assert a == b
        c = IndexedOrderedDict(b=2, a=1, c=3)
        assert a != c
        c.sort()
        assert a == c
        d = a.copy()
        assert a == d        

    def test_pickle(self):
        d = IndexedOrderedDict()
        d["foo"] = "bar"
        d["bar"] = "baz"

        pickled = pickle.dumps(d)
        unpickled = pickle.loads(pickled)

        assert d == unpickled

    def test_from_keys(self):
        d = IndexedOrderedDict.fromkeys({"a":'alpha',
            "b":'beta'}, "default")
        assert d.values() == ["default", "default"]
        
        d2 = IndexedOrderedDict.fromkeys(["a","b","c"], [])
        assert d2.values() == [[], [], []]
        d2["a"].append(1)
        assert d2.values() == [[1], [1], [1]]

    def test_or(self):
        d1 = IndexedOrderedDict(a=1, b=2)
        d2 = IndexedOrderedDict(b=3, c=4)
        d3 = d1 | d2
        assert d3 == IndexedOrderedDict(a=1, b=3, c=4)
        d1 |= d2
        assert d1 == IndexedOrderedDict(a=1, b=3, c=4)
    
    def test_update(self):
        d = IndexedOrderedDict(a=1, b=2)
        d.update({'b':3, 'c':4})
        assert d == IndexedOrderedDict(a=1, b=3, c=4)
        d.update(IndexedOrderedDict(c=3, b=2,d=5))
        assert d == IndexedOrderedDict(a=1, b=2, c=3,d=5)
        
if __name__ == "__main__":
    import os
    print(os.getpid())
    test = TestIndexedOrderedDict()
    test.test_init()