use std::collections::{HashMap, HashSet};
use crate::core::ast::IshValue;

#[derive(Clone, Debug, PartialEq)]
pub enum HeapObject {
    Array(Vec<IshValue>),
    List(Vec<IshValue>),
    Map(HashMap<String, IshValue>),
    Object {
        class_name: String,
        properties: HashMap<String, IshValue>,
    },
}

#[derive(Debug, Clone)]
pub struct Gobbler {
    pub young_heap: HashMap<usize, HeapObject>,
    pub old_heap: HashMap<usize, HeapObject>,
    pub object_ages: HashMap<usize, u8>,
    pub remembered_set: HashSet<usize>,
    pub next_id: usize,
    pub allocations_since_minor: usize,
    pub minor_threshold: usize,
    pub major_threshold: usize,
}

impl Gobbler {
    pub fn new() -> Self {
        Self {
            young_heap: HashMap::new(),
            old_heap: HashMap::new(),
            object_ages: HashMap::new(),
            remembered_set: HashSet::new(),
            next_id: 1, // Start from 1 so 0 can be used for null/uninitialized if needed, though IshValue::Null exists
            allocations_since_minor: 0,
            minor_threshold: 1000,
            major_threshold: 10000,
        }
    }

    pub fn allocate(&mut self, obj: HeapObject) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.young_heap.insert(id, obj);
        self.object_ages.insert(id, 0);
        self.allocations_since_minor += 1;
        id
    }

    pub fn get(&self, id: usize) -> Option<&HeapObject> {
        self.young_heap.get(&id).or_else(|| self.old_heap.get(&id))
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut HeapObject> {
        if self.old_heap.contains_key(&id) {
            self.remembered_set.insert(id);
            self.old_heap.get_mut(&id)
        } else {
            self.young_heap.get_mut(&id)
        }
    }

    pub fn free(&mut self, id: usize) {
        self.young_heap.remove(&id);
        self.old_heap.remove(&id);
        self.object_ages.remove(&id);
        self.remembered_set.remove(&id);
    }

    /// Mark and Sweep Garbage Collection
    /// Returns a list of (object_id, class_name, properties) for objects that were swept and might need their destructors called.
    pub fn collect(&mut self, stack_roots: &[HashMap<String, IshValue>], static_roots: &HashMap<String, IshValue>, return_value: Option<&IshValue>) -> Vec<(usize, String, HashMap<String, IshValue>)> {
        let mut marked = HashSet::new();
        let mut worklist = Vec::new();

        let major = self.old_heap.len() > self.major_threshold;

        // 1. Root Collection
        if let Some(IshValue::Reference(id)) = return_value {
            if marked.insert(*id) {
                worklist.push(*id);
            }
        }
        for scope in stack_roots {
            for (_, val) in scope {
                if let IshValue::Reference(id) = val {
                    if marked.insert(*id) {
                        worklist.push(*id);
                    }
                }
            }
        }
        for (_, val) in static_roots {
            if let IshValue::Reference(id) = val {
                if marked.insert(*id) {
                    worklist.push(*id);
                }
            }
        }

        if !major {
            for &id in &self.remembered_set {
                if marked.insert(id) {
                    worklist.push(id);
                }
            }
        }

        // 2. Mark Phase
        while let Some(current_id) = worklist.pop() {
            let obj_opt = self.young_heap.get(&current_id).or_else(|| self.old_heap.get(&current_id));
            if let Some(obj) = obj_opt {
                match obj {
                    HeapObject::Array(elements) | HeapObject::List(elements) => {
                        for val in elements {
                            if let IshValue::Reference(id) = val {
                                if marked.insert(*id) {
                                    worklist.push(*id);
                                }
                            }
                        }
                    }
                    HeapObject::Map(map) | HeapObject::Object { properties: map, .. } => {
                        for (_, val) in map {
                            if let IshValue::Reference(id) = val {
                                if marked.insert(*id) {
                                    worklist.push(*id);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Sweep Phase
        let mut finalized_objects = Vec::new();
        let mut young_to_sweep = Vec::new();
        let mut young_to_promote = Vec::new();

        for &id in self.young_heap.keys() {
            if !marked.contains(&id) {
                young_to_sweep.push(id);
            } else {
                if let Some(age) = self.object_ages.get_mut(&id) {
                    *age += 1;
                    if *age >= 3 {
                        young_to_promote.push(id);
                    }
                }
            }
        }

        for id in young_to_sweep {
            if let Some(HeapObject::Object { class_name, properties }) = self.young_heap.remove(&id) {
                finalized_objects.push((id, class_name, properties));
            } else {
                self.young_heap.remove(&id);
            }
            self.object_ages.remove(&id);
            self.remembered_set.remove(&id);
        }

        for id in young_to_promote {
            if let Some(obj) = self.young_heap.remove(&id) {
                self.old_heap.insert(id, obj);
            }
        }

        if major {
            let mut old_to_sweep = Vec::new();
            for &id in self.old_heap.keys() {
                if !marked.contains(&id) {
                    old_to_sweep.push(id);
                }
            }
            for id in old_to_sweep {
                if let Some(HeapObject::Object { class_name, properties }) = self.old_heap.remove(&id) {
                    finalized_objects.push((id, class_name, properties));
                } else {
                    self.old_heap.remove(&id);
                }
                self.object_ages.remove(&id);
                self.remembered_set.remove(&id);
            }
            self.remembered_set.clear();
        }

        self.allocations_since_minor = 0;

        finalized_objects
    }
}
