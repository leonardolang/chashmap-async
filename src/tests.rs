// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{CHashMap, MAX_LOAD_FACTOR_DENOM};
use std::cell::RefCell;
use std::sync::Arc;

use tokio::{task, test};

#[test]
async fn spam_insert() {
    let map = Arc::new(CHashMap::new());
    let mut joins = Vec::new();

    for t in 0..10 {
        let map = map.clone();
        joins.push(task::spawn(async move {
            for i in t * 1000..(t + 1) * 1000 {
                assert!(map.insert(i, !i).await.is_none());
                assert_eq!(map.insert(i, i).await.unwrap(), !i);
            }
        }));
    }

    for j in joins.drain(..) {
        j.await.unwrap();
    }

    for t in 0..5 {
        let map = map.clone();
        joins.push(task::spawn(async move {
            for i in t * 2000..(t + 1) * 2000 {
                assert_eq!(*map.get(&i).await.unwrap(), i);
            }
        }));
    }

    for j in joins {
        j.await.unwrap();
    }
}

#[test]
async fn spam_insert_new() {
    let m = Arc::new(CHashMap::new());
    let mut joins = Vec::new();

    for t in 0..10 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 1000..(t + 1) * 1000 {
                m.insert_new(i, i).await;
            }
        }));
    }

    for join in joins.drain(..) {
        join.await.unwrap();
    }

    for t in 0..5 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 2000..(t + 1) * 2000 {
                assert_eq!(*m.get(&i).await.unwrap(), i);
            }
        }));
    }

    for join in joins {
        join.await.unwrap();
    }
}

#[test]
async fn spam_upsert() {
    let m = Arc::new(CHashMap::new());
    let mut joins = Vec::new();

    for t in 0..10 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 1000..(t + 1) * 1000 {
                m.upsert(i, || !i, |_| unreachable!()).await;
                m.upsert(i, || unreachable!(), |x| *x = !*x).await;
            }
        }));
    }

    for j in joins.drain(..) {
        j.await.unwrap();
    }

    for t in 0..5 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 2000..(t + 1) * 2000 {
                assert_eq!(*m.get(&i).await.unwrap(), i);
            }
        }));
    }

    for j in joins {
        j.await.unwrap();
    }
}

#[test]
async fn spam_alter() {
    let m = Arc::new(CHashMap::new());
    let mut joins = Vec::new();

    for t in 0..10 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 1000..(t + 1) * 1000 {
                m.alter(i, |x| async move {
                    assert!(x.is_none());
                    Some(!i)
                })
                .await;
                m.alter(i, |x| async move {
                    assert_eq!(x, Some(!i));
                    Some(!x.unwrap())
                })
                .await;
            }
        }));
    }

    for j in joins.drain(..) {
        j.await.unwrap();
    }

    for t in 0..5 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            for i in t * 2000..(t + 1) * 2000 {
                assert_eq!(*m.get(&i).await.unwrap(), i);
                m.alter(i, |_| async { None }).await;
                assert!(m.get(&i).await.is_none());
            }
        }));
    }

    for j in joins {
        j.await.unwrap();
    }
}

#[test]
async fn lock_compete() {
    let m = Arc::new(CHashMap::new());

    m.insert("hey", "nah").await;

    let k = m.clone();
    let a = task::spawn(async move {
        *k.get_mut(&"hey").await.unwrap() = "hi";
    });
    let k = m.clone();
    let b = task::spawn(async move {
        *k.get_mut(&"hey").await.unwrap() = "hi";
    });

    a.await.unwrap();
    b.await.unwrap();

    assert_eq!(*m.get(&"hey").await.unwrap(), "hi");
}

#[test]
async fn simultanous_reserve() {
    let m = Arc::new(CHashMap::new());
    let mut joins = Vec::new();

    m.insert(1, 2).await;
    m.insert(3, 6).await;
    m.insert(8, 16).await;

    for _ in 0..10 {
        let m = m.clone();
        joins.push(task::spawn(async move {
            m.reserve(1000).await;
        }));
    }

    for j in joins {
        j.await.unwrap()
    }

    assert_eq!(*m.get(&1).await.unwrap(), 2);
    assert_eq!(*m.get(&3).await.unwrap(), 6);
    assert_eq!(*m.get(&8).await.unwrap(), 16);
}

#[test]
async fn create_capacity_zero() {
    let m = CHashMap::with_capacity(0);

    assert!(m.insert(1, 1).await.is_none());

    assert!(m.contains_key(&1).await);
    assert!(!m.contains_key(&0).await);
}

#[test]
async fn insert() {
    let m = CHashMap::new();
    assert_eq!(m.len(), 0);
    assert!(m.insert(1, 2).await.is_none());
    assert_eq!(m.len(), 1);
    assert!(m.insert(2, 4).await.is_none());
    assert_eq!(m.len(), 2);
    assert_eq!(*m.get(&1).await.unwrap(), 2);
    assert_eq!(*m.get(&2).await.unwrap(), 4);
}

#[test]
async fn upsert() {
    let m = CHashMap::new();
    assert_eq!(m.len(), 0);
    m.upsert(1, || 2, |_| unreachable!()).await;
    assert_eq!(m.len(), 1);
    m.upsert(2, || 4, |_| unreachable!()).await;
    assert_eq!(m.len(), 2);
    assert_eq!(*m.get(&1).await.unwrap(), 2);
    assert_eq!(*m.get(&2).await.unwrap(), 4);
}

#[test]
async fn upsert_update() {
    let m = CHashMap::new();
    m.insert(1, 2).await;
    m.upsert(1, || unreachable!(), |x| *x += 2).await;
    m.insert(2, 3).await;
    m.upsert(2, || unreachable!(), |x| *x += 3).await;
    assert_eq!(*m.get(&1).await.unwrap(), 4);
    assert_eq!(*m.get(&2).await.unwrap(), 6);
}

#[test]
async fn alter_string() {
    let m = CHashMap::new();
    assert_eq!(m.len(), 0);
    m.alter(1, |_| async { Some(String::new()) }).await;
    assert_eq!(m.len(), 1);
    m.alter(1, |x| async {
        let mut x = x.unwrap();
        x.push('a');
        Some(x)
    })
    .await;
    assert_eq!(m.len(), 1);
    assert_eq!(&*m.get(&1).await.unwrap(), "a");
}

#[test]
async fn clear() {
    let map = CHashMap::new();
    assert!(map.insert(1, 2).await.is_none());
    assert!(map.insert(2, 4).await.is_none());
    assert_eq!(map.len(), 2);

    let transfer = map.clear().await;
    assert_eq!(transfer.len(), 2);
    assert_eq!(*transfer.get(&1).await.unwrap(), 2);
    assert_eq!(*transfer.get(&2).await.unwrap(), 4);

    assert!(map.is_empty());
    assert_eq!(map.len(), 0);

    assert_eq!(map.get(&1).await, None);
    assert_eq!(map.get(&2).await, None);
}

#[test]
async fn clear_with_retain() {
    let m = CHashMap::new();
    assert!(m.insert(1, 2).await.is_none());
    assert!(m.insert(2, 4).await.is_none());
    assert_eq!(m.len(), 2);

    m.retain(|_, _| false).await;

    assert!(m.is_empty());
    assert_eq!(m.len(), 0);

    assert_eq!(m.get(&1).await, None);
    assert_eq!(m.get(&2).await, None);
}

#[test]
async fn retain() {
    let map = CHashMap::new();
    map.insert(1, 8).await;
    map.insert(2, 9).await;
    map.insert(3, 4).await;
    map.insert(4, 7).await;
    map.insert(5, 2).await;
    map.insert(6, 5).await;
    map.insert(7, 2).await;
    map.insert(8, 3).await;

    map.retain(|key, val| key & 1 == 0 && val & 1 == 1).await;

    assert_eq!(map.len(), 4);

    for (key, val) in map {
        assert_eq!(key & 1, 0);
        assert_eq!(val & 1, 1);
    }
}

thread_local! { static DROP_VECTOR: RefCell<Vec<isize>> = RefCell::new(Vec::new()) }

#[derive(Hash, PartialEq, Eq)]
struct Dropable {
    k: usize,
}

impl Dropable {
    fn new(k: usize) -> Dropable {
        DROP_VECTOR.with(|slot| {
            slot.borrow_mut()[k] += 1;
        });

        Dropable { k }
    }
}

impl Drop for Dropable {
    fn drop(&mut self) {
        DROP_VECTOR.with(|slot| {
            slot.borrow_mut()[self.k] -= 1;
        });
    }
}

impl Clone for Dropable {
    fn clone(&self) -> Dropable {
        Dropable::new(self.k)
    }
}

#[test]
async fn drops() {
    DROP_VECTOR.with(|slot| {
        *slot.borrow_mut() = vec![0; 200];
    });

    {
        let m = CHashMap::new();

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });

        for i in 0..100 {
            let d1 = Dropable::new(i);
            let d2 = Dropable::new(i + 100);
            m.insert(d1, d2).await;
        }

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 1);
            }
        });

        for i in 0..50 {
            let k = Dropable::new(i);
            let v = m.remove(&k).await;

            assert!(v.is_some());

            DROP_VECTOR.with(|v| {
                assert_eq!(v.borrow()[i], 1);
                assert_eq!(v.borrow()[i + 100], 1);
            });
        }

        DROP_VECTOR.with(|v| {
            for i in 0..50 {
                assert_eq!(v.borrow()[i], 0);
                assert_eq!(v.borrow()[i + 100], 0);
            }

            for i in 50..100 {
                assert_eq!(v.borrow()[i], 1);
                assert_eq!(v.borrow()[i + 100], 1);
            }
        });
    }

    DROP_VECTOR.with(|v| {
        for i in 0..200 {
            assert_eq!(v.borrow()[i], 0);
        }
    });
}

#[test]
async fn move_iter_drops() {
    DROP_VECTOR.with(|v| {
        *v.borrow_mut() = vec![0; 200];
    });

    let mut hm = {
        let hm = CHashMap::new();

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 0);
            }
        });

        for i in 0..100 {
            let d1 = Dropable::new(i);
            let d2 = Dropable::new(i + 100);
            hm.insert(d1, d2).await;
        }

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 1);
            }
        });

        hm
    };

    // By the way, ensure that cloning doesn't screw up the dropping.
    drop(hm.clone_mut());

    {
        let mut half = hm.into_iter().take(50);

        DROP_VECTOR.with(|v| {
            for i in 0..200 {
                assert_eq!(v.borrow()[i], 1);
            }
        });

        for _ in half.by_ref() {}

        DROP_VECTOR.with(|v| {
            let nk = (0..100).filter(|&i| v.borrow()[i] == 1).count();

            let nv = (0..100).filter(|&i| v.borrow()[i + 100] == 1).count();

            assert_eq!(nk, 50);
            assert_eq!(nv, 50);
        });
    };

    DROP_VECTOR.with(|v| {
        for i in 0..200 {
            assert_eq!(v.borrow()[i], 0);
        }
    });
}

#[test]
async fn empty_pop() {
    let map: CHashMap<isize, bool> = CHashMap::new();
    assert_eq!(map.remove(&0).await, None);
}

#[test]
#[ignore]
async fn lots_of_insertions() {
    let map = CHashMap::new();

    // Try this a few times to make sure we never screw up the hashmap's internal state.
    for _ in 0..10 {
        assert!(map.is_empty());

        for i in 1..1001 {
            assert!(map.insert(i, i).await.is_none());

            for j in 1..i + 1 {
                let r = map.get(&j).await;
                assert_eq!(*r.unwrap(), j);
            }

            for j in i + 1..1001 {
                let r = map.get(&j).await;
                assert_eq!(r, None);
            }
        }

        for i in 1001..2001 {
            assert!(!map.contains_key(&i).await);
        }

        // remove forwards
        for i in 1..1001 {
            assert!(map.remove(&i).await.is_some());

            for j in 1..i + 1 {
                assert!(!map.contains_key(&j).await);
            }

            for j in i + 1..1001 {
                assert!(map.contains_key(&j).await);
            }
        }

        for i in 1..1001 {
            assert!(!map.contains_key(&i).await);
        }

        for i in 1..1001 {
            assert!(map.insert(i, i).await.is_none());
        }

        // remove backwards
        for i in (1..1001).rev() {
            assert!(map.remove(&i).await.is_some());

            for j in i..1001 {
                assert!(!map.contains_key(&j).await);
            }

            for j in 1..i {
                assert!(map.contains_key(&j).await);
            }
        }
    }
}

#[test]
async fn find_mut() {
    let map = CHashMap::new();
    assert!(map.insert(1, 12).await.is_none());
    assert!(map.insert(2, 8).await.is_none());
    assert!(map.insert(5, 14).await.is_none());
    let new = 100;
    match map.get_mut(&5).await {
        None => panic!("Entry is empty"),
        Some(mut x) => *x = new,
    }
    assert_eq!(*map.get(&5).await.unwrap(), new);
}

#[test]
async fn insert_overwrite() {
    let map = CHashMap::new();
    assert_eq!(map.len(), 0);
    assert!(map.insert(1, 2).await.is_none());
    assert_eq!(map.len(), 1);
    assert_eq!(*map.get(&1).await.unwrap(), 2);
    assert_eq!(map.len(), 1);
    assert!(!map.insert(1, 3).await.is_none());
    assert_eq!(map.len(), 1);
    assert_eq!(*map.get(&1).await.unwrap(), 3);
}

#[test]
async fn insert_conflicts() {
    let map = CHashMap::with_capacity(4);
    assert!(map.insert(1, 2).await.is_none());
    assert!(map.insert(5, 3).await.is_none());
    assert!(map.insert(9, 4).await.is_none());
    assert_eq!(*map.get(&9).await.unwrap(), 4);
    assert_eq!(*map.get(&5).await.unwrap(), 3);
    assert_eq!(*map.get(&1).await.unwrap(), 2);
}

#[test]
async fn conflict_remove() {
    let map = CHashMap::with_capacity(4);
    assert!(map.insert(1, 2).await.is_none());
    assert_eq!(*map.get(&1).await.unwrap(), 2);
    assert!(map.insert(5, 3).await.is_none());
    assert_eq!(*map.get(&1).await.unwrap(), 2);
    assert_eq!(*map.get(&5).await.unwrap(), 3);
    assert!(map.insert(9, 4).await.is_none());
    assert_eq!(*map.get(&1).await.unwrap(), 2);
    assert_eq!(*map.get(&5).await.unwrap(), 3);
    assert_eq!(*map.get(&9).await.unwrap(), 4);
    assert!(map.remove(&1).await.is_some());
    assert_eq!(*map.get(&9).await.unwrap(), 4);
    assert_eq!(*map.get(&5).await.unwrap(), 3);
}

#[test]
async fn is_empty() {
    let map = CHashMap::with_capacity(4);
    assert!(map.insert(1, 2).await.is_none());
    assert!(!map.is_empty());
    assert!(map.remove(&1).await.is_some());
    assert!(map.is_empty());
}

#[test]
async fn pop() {
    let map = CHashMap::new();
    map.insert(1, 2).await;
    assert_eq!(map.remove(&1).await, Some(2));
    assert_eq!(map.remove(&1).await, None);
}

#[test]
async fn find() {
    let m = CHashMap::new();
    assert!(m.get(&1).await.is_none());
    m.insert(1, 2).await;
    let lock = m.get(&1).await;
    match lock {
        None => panic!("Entry is empty"),
        Some(v) => assert_eq!(*v, 2),
    }
}

#[test]
async fn known_size_with_capacity_matches_shrink() {
    for i in 0..(MAX_LOAD_FACTOR_DENOM * 4) {
        // Setup a map where we know the number of entries we will store
        let map = CHashMap::with_capacity(i);
        let original_capacity = map.capacity().await;
        let buckets = map.buckets().await;
        assert!(
            i <= original_capacity,
            "Expected {} <= {} for {} buckets",
            i,
            original_capacity,
            buckets
        );
        for j in 0..i {
            map.insert(j, j).await;
        }

        // Make sure inserting didn't increase capacity given we already knew
        // number of entries planned on map construction
        let grown_capacity = map.capacity().await;
        assert_eq!(original_capacity, grown_capacity, " for {} inserts", i);

        // Shrink it and check that capacity is the same
        map.shrink_to_fit().await;
        let shrunken_capacity = map.capacity().await;
        assert_eq!(
            shrunken_capacity, original_capacity,
            "Expected {} == {} ",
            shrunken_capacity, shrunken_capacity
        );
    }
}

#[test]
async fn shrink_to_fit_after_insert() {
    for i in 0..(MAX_LOAD_FACTOR_DENOM * 4) {
        // Setup
        let map = CHashMap::new();
        for j in 0..i {
            map.insert(j, j).await;
        }
        let original_capacity = map.capacity().await;

        // Test
        map.shrink_to_fit().await;
        let shrunken_capacity = map.capacity().await;
        assert!(
            shrunken_capacity <= original_capacity,
            "Unexpected capacity after shrink given {} inserts. Expected {} <= {}",
            i,
            shrunken_capacity,
            original_capacity
        );
    }
}

#[test]
async fn reserve_shrink_to_fit() {
    let map = CHashMap::new();
    map.insert(0, 0).await;
    map.remove(&0).await;
    assert!(map.capacity().await >= map.len());
    for i in 0..128 {
        map.insert(i, i).await;
    }
    map.reserve(256).await;

    let usable_cap = map.capacity().await;
    for i in 128..(128 + 256) {
        map.insert(i, i).await;
        assert_eq!(map.capacity().await, usable_cap);
    }

    for i in 100..(128 + 256) {
        assert_eq!(map.remove(&i).await, Some(i));
    }
    map.shrink_to_fit().await;

    assert_eq!(map.len(), 100);
    assert!(!map.is_empty());
    assert!(map.capacity().await >= map.len());

    for i in 0..100 {
        assert_eq!(map.remove(&i).await, Some(i));
    }
    map.shrink_to_fit().await;
    map.insert(0, 0).await;

    assert_eq!(map.len(), 1);
    assert!(map.capacity().await >= map.len());
    assert_eq!(map.remove(&0).await, Some(0));
}

#[test]
async fn from_iter() {
    let xs = [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)];

    let map: CHashMap<_, _> = xs.iter().cloned().collect();

    for &(k, v) in &xs {
        assert_eq!(*map.get(&k).await.unwrap(), v);
    }
}

#[test]
async fn capacity_not_less_than_len() {
    let a = CHashMap::new();
    let mut item = 0;

    for _ in 0..116 {
        a.insert(item, 0).await;
        item += 1;
    }

    assert!(a.capacity().await > a.len());

    let free = a.capacity().await - a.len();
    for _ in 0..free {
        a.insert(item, 0).await;
        item += 1;
    }

    assert_eq!(a.len(), a.capacity().await);

    // Insert at capacity should cause allocation.
    a.insert(item, 0).await;
    assert!(a.capacity().await > a.len());
}

#[test]
async fn insert_into_map_full_of_free_buckets() {
    let m = CHashMap::with_capacity(1);
    for i in 0..100 {
        m.insert(i, 0).await;
        m.remove(&i).await;
    }
}

#[test]
async fn lookup_borrowed() {
    let m = CHashMap::with_capacity(1);
    m.insert("v".to_owned(), "value").await;
    m.get("v").await.unwrap();
}
