use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};

#[derive(Pod, Debug, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct LinearBlockAllocation {
    start: u64,
    end: u64,
}

impl LinearBlockAllocation {
    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }
}

pub struct LinearBlockAllocator {
    free_regions: BTreeMap<u64, u64>, // start -> size
    used_regions: BTreeMap<u64, u64>, // start -> size
    used_bytes: u64,
    total_bytes: u64,
}

impl LinearBlockAllocator {
    pub fn new(total_size: u64) -> Self {
        let mut free_regions = BTreeMap::new();
        free_regions.insert(0, total_size);
        Self {
            free_regions,
            used_regions: BTreeMap::new(),
            used_bytes: 0,
            total_bytes: total_size,
        }
    }

    pub fn allocate(&mut self, size: u64) -> Option<LinearBlockAllocation> {
        println!("USAGE: {} / {}", self.used_bytes, self.total_bytes);

        for (&start, &region_size) in &self.free_regions {
            if region_size >= size {
                let end = start + size;
                self.free_regions.remove(&start);
                if region_size > size {
                    self.free_regions.insert(end, region_size - size);
                }
                self.used_regions.insert(start, size);

                self.used_bytes += size;

                return Some(LinearBlockAllocation { start, end });
            }
        }

        None
    }

    pub fn free(&mut self, allocation: &LinearBlockAllocation) {
        let start = allocation.start;
        let size = allocation.end - allocation.start;
        if self.used_regions.remove(&start).is_some() {
            self.insert_free_region(start, size);
            self.used_bytes -= size;
        } else {
            panic!("Attempted to free unallocated region: {:?}", allocation);
        }
    }

    fn insert_free_region(&mut self, start: u64, size: u64) {
        // Optional: merge with neighboring free regions
        let mut new_start = start;
        let mut new_end = start + size;

        if let Some((&prev_start, &_prev_size)) = self
            .free_regions
            .range(..start)
            .rev()
            .find(|(&s, &sz)| s + sz == start)
        {
            self.free_regions.remove(&prev_start);
            new_start = prev_start;
        }

        if let Some((&next_start, &next_size)) = self
            .free_regions
            .range(start + size..)
            .find(|(&s, _)| s == new_end)
        {
            self.free_regions.remove(&next_start);
            new_end = next_start + next_size;
        }

        self.free_regions.insert(new_start, new_end - new_start);
    }
}
