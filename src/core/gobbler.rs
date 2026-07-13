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
    pub heap: HashMap<usize, HeapObject>,
    next_id: usize,
}

impl Gobbler {
    pub fn new() -> Self {
        Self {
            heap: HashMap::new(),
            next_id: 1, // Start from 1 so 0 can be used for null/uninitialized if needed, though IshValue::Null exists
        }
    }

    pub fn allocate(&mut self, obj: HeapObject) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.heap.insert(id, obj);
        id
    }

    pub fn get(&self, id: usize) -> Option<&HeapObject> {
        self.heap.get(&id)
    }

    pub fn get_mut(&mut self, id: usize) -> Option<&mut HeapObject> {
        self.heap.get_mut(&id)
    }

    pub fn free(&mut self, id: usize) {
        self.heap.remove(&id);
    }

    /// Mark and Sweep Garbage Collection
    /// Returns a list of (object_id, class_name, properties) for objects that were swept and might need their destructors called.
    pub fn collect(&mut self, stack_roots: &[HashMap<String, IshValue>], static_roots: &HashMap<String, IshValue>, return_value: Option<&IshValue>) -> Vec<(usize, String, HashMap<String, IshValue>)> {
        let mut marked = HashSet::new();
        let mut worklist = Vec::new();

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

        // 2. Mark Phase
        while let Some(current_id) = worklist.pop() {
            if let Some(obj) = self.heap.get(&current_id) {
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
        let mut to_sweep = Vec::new();
        for &id in self.heap.keys() {
            if !marked.contains(&id) {
                to_sweep.push(id);
            }
        }

        let mut finalized_objects = Vec::new();

        for id in to_sweep {
            // Before removing, check if it's an Object (needs destructor potentially)
            if let Some(HeapObject::Object { class_name, properties }) = self.heap.get(&id) {
                finalized_objects.push((id, class_name.clone(), properties.clone()));
            }
            self.heap.remove(&id);
        }

        finalized_objects
    }
}
