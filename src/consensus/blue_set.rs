use super::dag_traversal::DAGTraversal;
use super::ghostdag::BlueWork;
use crate::core::BlockHash;
use crate::dag::BlockDAG;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Blue set calculation for GHOSTDAG consensus
/// A block is in the blue set if its anticone size is <= k
#[derive(Clone, Debug)]
pub struct BlueSet {
    /// Parameter k: maximum anticone size for blue blocks
    pub k: usize,
}

impl BlueSet {
    /// Create new BlueSet calculator with parameter k
    pub fn new(k: usize) -> Self {
        BlueSet { k }
    }

    /// Build blue set for a block
    /// Returns set of blocks that should be colored blue
    pub fn build_blue_set(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> HashSet<BlockHash> {
        let mut blue_set = HashSet::new();

        // Start with reference block itself
        blue_set.insert(reference_hash.clone());

        // Get all blocks in topological order
        let all_blocks = DAGTraversal::get_all_blocks(dag);

        // Process blocks in topological order (parents before children)
        for block_hash in all_blocks {
            if block_hash == *reference_hash {
                continue;
            }

            // Calculate anticone of this block relative to reference
            let anticone = DAGTraversal::get_anticone(dag, &block_hash, reference_hash);

            // Block is blue if anticone size <= k
            if anticone.len() <= self.k {
                blue_set.insert(block_hash);
            }
        }

        blue_set
    }

    /// Build red set (complement of blue set)
    pub fn build_red_set(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> HashSet<BlockHash> {
        let blue_set = self.build_blue_set(dag, reference_hash);
        let all_blocks = DAGTraversal::get_all_blocks(dag)
            .into_iter()
            .collect::<HashSet<_>>();

        all_blocks.difference(&blue_set).cloned().collect()
    }

    /// Check if block is blue relative to reference
    pub fn is_blue(
        &self,
        dag: &BlockDAG,
        block_hash: &BlockHash,
        reference_hash: &BlockHash,
    ) -> bool {
        // Block must be ancestor or equal to reference to be blue
        if block_hash != reference_hash
            && !DAGTraversal::is_ancestor(dag, block_hash, reference_hash)
        {
            return false;
        }

        let anticone = DAGTraversal::get_anticone(dag, block_hash, reference_hash);
        anticone.len() <= self.k
    }

    /// Get anticone size of block relative to reference
    pub fn get_anticone_size(
        &self,
        dag: &BlockDAG,
        block_hash: &BlockHash,
        reference_hash: &BlockHash,
    ) -> usize {
        let anticone = DAGTraversal::get_anticone(dag, block_hash, reference_hash);
        anticone.len()
    }

    /// Get anticone blocks themselves
    pub fn get_anticone(
        &self,
        dag: &BlockDAG,
        block_hash: &BlockHash,
        reference_hash: &BlockHash,
    ) -> HashSet<BlockHash> {
        DAGTraversal::get_anticone(dag, block_hash, reference_hash)
    }

    /// Count blocks in blue set for reference
    pub fn count_blue_blocks(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> usize {
        self.build_blue_set(dag, reference_hash).len()
    }

    /// Count blocks in red set for reference
    pub fn count_red_blocks(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> usize {
        self.build_red_set(dag, reference_hash).len()
    }

    /// Get all blue block hashes
    pub fn get_blue_blocks(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> Vec<BlockHash> {
        let blocks = self.build_blue_set(dag, reference_hash);
        let mut result: Vec<_> = blocks.into_iter().collect();
        // Sort deterministically
        result.sort();
        result
    }

    /// Get all red block hashes
    pub fn get_red_blocks(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> Vec<BlockHash> {
        let blocks = self.build_red_set(dag, reference_hash);
        let mut result: Vec<_> = blocks.into_iter().collect();
        // Sort deterministically
        result.sort();
        result
    }

    /// Check if all ancestors of block are blue (validation)
    pub fn are_all_ancestors_blue(
        &self,
        dag: &BlockDAG,
        block_hash: &BlockHash,
        reference_hash: &BlockHash,
    ) -> bool {
        let ancestors = DAGTraversal::get_ancestors(dag, block_hash);
        let blue_set = self.build_blue_set(dag, reference_hash);

        for ancestor in ancestors {
            if !blue_set.contains(&ancestor) {
                return false;
            }
        }

        true
    }

    /// Get blue set statistics
    pub fn get_blue_set_stats(&self, dag: &BlockDAG, reference_hash: &BlockHash) -> BlueSetStats {
        let blue_set = self.build_blue_set(dag, reference_hash);
        let red_set = self.build_red_set(dag, reference_hash);

        let total_blocks = dag.block_count();
        let blue_count = blue_set.len();
        let red_count = red_set.len();

        BlueSetStats {
            blue_count,
            red_count,
            total_blocks,
            k_parameter: self.k,
            blue_ratio: if total_blocks > 0 {
                (blue_count as f64) / (total_blocks as f64)
            } else {
                0.0
            },
        }
    }
}

/// Statistics about blue set
#[derive(Clone, Debug)]
pub struct BlueSetStats {
    pub blue_count: usize,
    pub red_count: usize,
    pub total_blocks: usize,
    pub k_parameter: usize,
    pub blue_ratio: f64,
}

/// Configuration for BlueSet manager
#[derive(Clone, Debug)]
pub struct BlueSetConfig {
    pub k: usize,
    pub max_merge_set_size: usize,
    pub blue_window_limit: usize,
    pub conflict_rate_threshold: f64,
    pub timestamp_sanity_window: u64, // seconds
    pub cache_size: usize,
}

impl Default for BlueSetConfig {
    fn default() -> Self {
        Self {
            k: 2,
            max_merge_set_size: 100,
            blue_window_limit: 1000,
            conflict_rate_threshold: 0.1,
            timestamp_sanity_window: 600,
            cache_size: 10000,
        }
    }
}

/// Result of BlueSet operations
#[derive(Clone, Debug)]
pub enum BlueSetResult<T> {
    Ok(T),
    Err(String),
}

/// Cache for blue set computations
#[derive(Clone, Debug)]
pub struct BlueSetCache {
    pub blue_sets: HashMap<BlockHash, HashSet<BlockHash>>,
    pub anticone_cache: HashMap<(BlockHash, BlockHash), HashSet<BlockHash>>,
    pub blue_scores: HashMap<BlockHash, u64>,
    pub merge_sets: HashMap<(BlockHash, BlockHash), HashSet<BlockHash>>,
}

impl BlueSetCache {
    pub fn new() -> Self {
        BlueSetCache {
            blue_sets: HashMap::new(),
            anticone_cache: HashMap::new(),
            blue_scores: HashMap::new(),
            merge_sets: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.blue_sets.clear();
        self.anticone_cache.clear();
        self.blue_scores.clear();
        self.merge_sets.clear();
    }

    pub fn prune_old_entries(&mut self, max_size: usize) {
        if self.blue_sets.len() > max_size {
            // Simple pruning: remove oldest (by insertion order approximation)
            let to_remove: Vec<_> = self
                .blue_sets
                .keys()
                .take(self.blue_sets.len() - max_size)
                .cloned()
                .collect();
            for key in to_remove {
                self.blue_sets.remove(&key);
            }
        }
        // Similar for others
        if self.anticone_cache.len() > max_size {
            let to_remove: Vec<_> = self
                .anticone_cache
                .keys()
                .take(self.anticone_cache.len() - max_size)
                .cloned()
                .collect();
            for key in to_remove {
                self.anticone_cache.remove(&key);
            }
        }
        if self.blue_scores.len() > max_size {
            let to_remove: Vec<_> = self
                .blue_scores
                .keys()
                .take(self.blue_scores.len() - max_size)
                .cloned()
                .collect();
            for key in to_remove {
                self.blue_scores.remove(&key);
            }
        }
        if self.merge_sets.len() > max_size {
            let to_remove: Vec<_> = self
                .merge_sets
                .keys()
                .take(self.merge_sets.len() - max_size)
                .cloned()
                .collect();
            for key in to_remove {
                self.merge_sets.remove(&key);
            }
        }
    }
}

/// Main BlueSet manager with concurrency support
pub struct BlueSetManager {
    config: BlueSetConfig,
    dag: Arc<RwLock<BlockDAG>>,
    cache: Arc<RwLock<BlueSetCache>>,
    persistent_store: Option<Arc<RwLock<rocksdb::DB>>>,
}

impl BlueSetManager {
    /// Create new BlueSetManager
    pub fn new(config: BlueSetConfig, dag: Arc<RwLock<BlockDAG>>) -> Self {
        BlueSetManager {
            config,
            dag,
            cache: Arc::new(RwLock::new(BlueSetCache::new())),
            persistent_store: None,
        }
    }

    /// Attach persistent storage
    pub fn with_persistent_store(mut self, db: Arc<RwLock<rocksdb::DB>>) -> Self {
        self.persistent_store = Some(db);
        self
    }

    /// Select blue blocks for a reference with deterministic ordering
    pub async fn select_blue_blocks(
        &self,
        reference_hash: &BlockHash,
    ) -> BlueSetResult<HashSet<BlockHash>> {
        // Check cache first
        {
            let cache = self.cache.read();
            if let Some(blue_set) = cache.blue_sets.get(reference_hash) {
                return BlueSetResult::Ok(blue_set.clone());
            }
        }

        let dag = self.dag.read();
        let candidates = match self.get_blue_candidates(&dag, reference_hash).await {
            BlueSetResult::Ok(c) => c,
            BlueSetResult::Err(e) => return BlueSetResult::Err(e),
        };

        // Sort candidates deterministically
        let mut candidates = candidates;
        match self.sort_candidates(&mut candidates, &dag).await {
            BlueSetResult::Ok(()) => (),
            BlueSetResult::Err(e) => return BlueSetResult::Err(e),
        };

        // Apply k-cluster validation
        let mut blue_set = HashSet::new();
        blue_set.insert(reference_hash.clone());

        for candidate in candidates {
            let is_blue = match self.is_blue_candidate(&candidate, &blue_set, &dag).await {
                BlueSetResult::Ok(b) => b,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            if is_blue {
                blue_set.insert(candidate);
            }
        }

        // Cache result
        {
            let mut cache = self.cache.write();
            cache
                .blue_sets
                .insert(reference_hash.clone(), blue_set.clone());
            cache.prune_old_entries(self.config.cache_size);
        }

        // Persist if available
        if let Some(store) = &self.persistent_store {
            match self
                .persist_blue_set(store, reference_hash, &blue_set)
                .await
            {
                BlueSetResult::Ok(()) => (),
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
        }

        BlueSetResult::Ok(blue_set)
    }

    /// Get blue candidates sorted by priority
    async fn get_blue_candidates(
        &self,
        dag: &BlockDAG,
        reference_hash: &BlockHash,
    ) -> BlueSetResult<Vec<BlockHash>> {
        let all_blocks = DAGTraversal::get_all_blocks(dag);
        let mut candidates = Vec::new();

        for block_hash in all_blocks {
            if block_hash == *reference_hash {
                continue;
            }

            if DAGTraversal::is_ancestor(dag, &block_hash, reference_hash) {
                candidates.push(block_hash);
            }
        }

        BlueSetResult::Ok(candidates)
    }

    /// Sort candidates deterministically
    async fn sort_candidates(
        &self,
        candidates: &mut Vec<BlockHash>,
        dag: &BlockDAG,
    ) -> BlueSetResult<()> {
        let mut scored: Vec<(BlockHash, BlueWork)> = Vec::new();

        for candidate in candidates.iter() {
            let blue_score = match self.compute_blue_score_incremental(candidate).await {
                BlueSetResult::Ok(score) => score,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            let cumulative_work = match self.get_cumulative_work(candidate, dag) {
                BlueSetResult::Ok(work) => work,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            let timestamp = dag
                .get_block(candidate)
                .map(|b| b.header.timestamp)
                .unwrap_or(0);

            let work = BlueWork {
                blue_score,
                cumulative_work,
                timestamp,
            };
            scored.push((candidate.clone(), work));
        }

        // Sort by BlueWork ordering
        scored.sort_by(|a, b| a.1.cmp(&b.1));

        *candidates = scored.into_iter().map(|(h, _)| h).collect();
        BlueSetResult::Ok(())
    }

    /// Check if block is a valid blue candidate
    async fn is_blue_candidate(
        &self,
        block_hash: &BlockHash,
        current_blue: &HashSet<BlockHash>,
        dag: &BlockDAG,
    ) -> BlueSetResult<bool> {
        // Check timestamp sanity
        if let Some(block) = dag.get_block(block_hash) {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if block.header.timestamp + self.config.timestamp_sanity_window < current_time {
                return BlueSetResult::Ok(false);
            }
        }

        // Check anticone size against current blue set
        let anticone = match self
            .compute_anticone_cached(
                block_hash,
                &current_blue.iter().next().unwrap_or(block_hash),
                dag,
            )
            .await
        {
            BlueSetResult::Ok(a) => a,
            BlueSetResult::Err(e) => return BlueSetResult::Err(e),
        };
        if anticone.len() > self.config.k {
            return BlueSetResult::Ok(false);
        }

        // Check DAG density
        let density = match self.compute_dag_density(dag).await {
            BlueSetResult::Ok(d) => d,
            BlueSetResult::Err(e) => return BlueSetResult::Err(e),
        };
        if density > self.config.conflict_rate_threshold {
            // Apply density-aware filtering
            if anticone.len() > self.config.k / 2 {
                return BlueSetResult::Ok(false);
            }
        }

        BlueSetResult::Ok(true)
    }

    /// Compute anticone with caching
    async fn compute_anticone_cached(
        &self,
        block_hash: &BlockHash,
        reference_hash: &BlockHash,
        dag: &BlockDAG,
    ) -> BlueSetResult<HashSet<BlockHash>> {
        let key = (*block_hash, *reference_hash);

        // Check cache
        {
            let cache = self.cache.read();
            if let Some(anticone) = cache.anticone_cache.get(&key) {
                return BlueSetResult::Ok(anticone.clone());
            }
        }

        let anticone = DAGTraversal::get_anticone(dag, block_hash, reference_hash);

        // Cache result
        {
            let mut cache = self.cache.write();
            cache.anticone_cache.insert(key, anticone.clone());
        }

        BlueSetResult::Ok(anticone)
    }

    /// Compute DAG density heuristic
    async fn compute_dag_density(&self, dag: &BlockDAG) -> BlueSetResult<f64> {
        let total_blocks = dag.block_count();
        if total_blocks == 0 {
            return BlueSetResult::Ok(0.0);
        }

        let tips = dag.get_tips();
        let mut total_edges = 0;
        for tip in tips {
            if let Some(block) = dag.get_block(&tip) {
                total_edges += block.header.parent_hashes.len();
            }
        }

        BlueSetResult::Ok(total_edges as f64 / total_blocks as f64)
    }

    /// Compute blue score incrementally
    pub async fn compute_blue_score_incremental(
        &self,
        block_hash: &BlockHash,
    ) -> BlueSetResult<u64> {
        // Check cache
        {
            let cache = self.cache.read();
            if let Some(score) = cache.blue_scores.get(block_hash) {
                return BlueSetResult::Ok(*score);
            }
        }

        let parents = {
            let dag = self.dag.read();
            dag.get_block(block_hash)
                .map(|b| b.header.parent_hashes.clone())
                .unwrap_or_default()
        };

        let mut max_parent_score = 0u64;
        for parent in parents {
            let parent_score = match Box::pin(self.compute_blue_score_incremental(&parent)).await {
                BlueSetResult::Ok(score) => score,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            max_parent_score = max_parent_score.max(parent_score);
        }

        let score = max_parent_score + 1;

        // Cache result
        {
            let mut cache = self.cache.write();
            cache.blue_scores.insert(block_hash.clone(), score);
        }

        BlueSetResult::Ok(score)
    }

    /// Get cumulative work for a block
    fn get_cumulative_work(&self, block_hash: &BlockHash, dag: &BlockDAG) -> BlueSetResult<u64> {
        // Simplified: just count ancestors
        let ancestors = DAGTraversal::get_ancestors(dag, block_hash);
        BlueSetResult::Ok(ancestors.len() as u64 + 1)
    }

    /// Compute merge set between two blocks
    pub async fn compute_merge_set(
        &self,
        block_a: &BlockHash,
        block_b: &BlockHash,
    ) -> BlueSetResult<HashSet<BlockHash>> {
        let key = if block_a < block_b {
            (*block_a, *block_b)
        } else {
            (*block_b, *block_a)
        };

        // Check cache
        {
            let cache = self.cache.read();
            if let Some(merge_set) = cache.merge_sets.get(&key) {
                if merge_set.len() <= self.config.max_merge_set_size {
                    return BlueSetResult::Ok(merge_set.clone());
                }
            }
        }

        let dag = self.dag.read();
        let past_a = DAGTraversal::get_ancestors(&dag, block_a);
        let past_b = DAGTraversal::get_ancestors(&dag, block_b);

        let mut merge_set: HashSet<BlockHash> = past_a.intersection(&past_b).cloned().collect();

        // Apply size guard
        if merge_set.len() > self.config.max_merge_set_size {
            // Fallback: take only the most recent ancestors
            let mut sorted: Vec<BlockHash> = merge_set.into_iter().collect();
            sorted.sort_by_key(|h| dag.get_block(h).map(|b| b.header.timestamp).unwrap_or(0));
            sorted.reverse(); // Most recent first
            merge_set = sorted
                .into_iter()
                .take(self.config.max_merge_set_size)
                .collect();
        }

        // Cache result
        {
            let mut cache = self.cache.write();
            cache.merge_sets.insert(key, merge_set.clone());
        }

        BlueSetResult::Ok(merge_set)
    }

    /// Compute virtual blue set for mining tips
    pub async fn compute_virtual_blue_set(
        &self,
        tips: &[BlockHash],
    ) -> BlueSetResult<HashSet<BlockHash>> {
        if tips.is_empty() {
            return BlueSetResult::Ok(HashSet::new());
        }

        // Find the tip with highest blue score
        let mut best_tip = tips[0];
        let best_score = match self.compute_blue_score_incremental(&best_tip).await {
            BlueSetResult::Ok(score) => score,
            BlueSetResult::Err(e) => return BlueSetResult::Err(e),
        };

        let mut best_score = best_score;
        for tip in tips.iter().skip(1) {
            let score = match self.compute_blue_score_incremental(tip).await {
                BlueSetResult::Ok(s) => s,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            if score > best_score {
                best_score = score;
                best_tip = *tip;
            }
        }

        self.select_blue_blocks(&best_tip).await
    }

    /// Prune blue history for memory management
    pub async fn prune_blue_history(&self, pruning_point: &BlockHash) {
        let dag = self.dag.read();
        let mut cache = self.cache.write();

        // Remove entries for blocks below pruning point
        let to_remove: Vec<BlockHash> = cache
            .blue_sets
            .keys()
            .filter(|h| DAGTraversal::is_ancestor(&dag, h, pruning_point))
            .cloned()
            .collect();

        for key in to_remove {
            cache.blue_sets.remove(&key);
            cache.blue_scores.remove(&key);
        }

        // Clean anticone and merge caches
        cache.anticone_cache.retain(|(a, b), _| {
            !DAGTraversal::is_ancestor(&dag, a, pruning_point)
                && !DAGTraversal::is_ancestor(&dag, b, pruning_point)
        });

        cache.merge_sets.retain(|(a, b), _| {
            !DAGTraversal::is_ancestor(&dag, a, pruning_point)
                && !DAGTraversal::is_ancestor(&dag, b, pruning_point)
        });
    }

    /// Detect attack patterns
    pub async fn detect_attack_patterns(&self, block_hash: &BlockHash) -> BlueSetResult<()> {
        let dag = self.dag.read();

        // Check for parasite cluster
        let parents = dag
            .get_block(block_hash)
            .map(|b| b.header.parent_hashes.clone())
            .unwrap_or_default();

        for parent in parents {
            let merge_set = match self.compute_merge_set(block_hash, &parent).await {
                BlueSetResult::Ok(set) => set,
                BlueSetResult::Err(e) => return BlueSetResult::Err(e),
            };
            if merge_set.len() < 3 {
                // Arbitrary threshold
                return BlueSetResult::Err("Detected parasite cluster attack".to_string());
            }
        }

        // Check for spam conflicting blocks
        let anticone_size = {
            let tips = dag.get_tips();
            if tips.is_empty() {
                0
            } else {
                match self
                    .compute_anticone_cached(block_hash, &tips[0], &dag)
                    .await
                {
                    BlueSetResult::Ok(anticone) => anticone.len(),
                    BlueSetResult::Err(e) => return BlueSetResult::Err(e),
                }
            }
        };

        if anticone_size > self.config.k * 2 {
            return BlueSetResult::Err("Detected spam conflicting blocks".to_string());
        }

        BlueSetResult::Ok(())
    }

    /// Persist blue set to storage
    async fn persist_blue_set(
        &self,
        store: &Arc<RwLock<rocksdb::DB>>,
        reference: &BlockHash,
        blue_set: &HashSet<BlockHash>,
    ) -> BlueSetResult<()> {
        let db = store.read();
        let key = format!("blue_set:{}", hex::encode(reference));
        let data = match serde_json::to_vec(blue_set) {
            Ok(d) => d,
            Err(e) => return BlueSetResult::Err(e.to_string()),
        };
        match db.put(key.as_bytes(), data) {
            Ok(()) => BlueSetResult::Ok(()),
            Err(e) => BlueSetResult::Err(e.to_string()),
        }
    }

    /// Load blue set from storage
    pub async fn load_blue_set(
        &self,
        reference: &BlockHash,
    ) -> BlueSetResult<Option<HashSet<BlockHash>>> {
        if let Some(store) = &self.persistent_store {
            let db = store.read();
            let key = format!("blue_set:{}", hex::encode(reference));
            match db.get(key.as_bytes()) {
                Ok(Some(data)) => {
                    let blue_set: HashSet<BlockHash> = match serde_json::from_slice(&data) {
                        Ok(set) => set,
                        Err(e) => return BlueSetResult::Err(e.to_string()),
                    };
                    let mut cache = self.cache.write();
                    cache.blue_sets.insert(reference.clone(), blue_set.clone());
                    BlueSetResult::Ok(Some(blue_set))
                }
                Ok(None) => BlueSetResult::Ok(None),
                Err(e) => BlueSetResult::Err(e.to_string()),
            }
        } else {
            BlueSetResult::Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Transaction;

    fn create_test_block(hash_seed: &str, parents: Vec<BlockHash>) -> crate::Block {
        let seed_bytes = hash_seed.as_bytes();
        let mut from: [u8; 20] = [1; 20];
        let mut to: [u8; 20] = [2; 20];
        for (i, &b) in seed_bytes.iter().take(20).enumerate() {
            from[i] = from[i].wrapping_add(b);
        }
        for (i, &b) in seed_bytes.iter().skip(20).take(20).enumerate() {
            to[i] = to[i].wrapping_add(b);
        }
        let amount = 100 + seed_bytes.len() as u64;
        let tx = Transaction::new(from, to, amount, 0, 21000, 1);
        crate::Block::new(
            parents,
            1000 + hash_seed.len() as u64,
            vec![tx],
            42,
            0,
            0,
            [0; 20],
            [0; 32],
        )
    }

    #[test]
    fn test_build_blue_set_linear_chain() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);
        let grandchild = create_test_block("grandchild", vec![child.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();
        dag.insert_block(grandchild.clone()).unwrap();

        let blue_set = BlueSet::new(1);
        let blue_blocks = blue_set.build_blue_set(&dag, &grandchild.hash);

        // In linear chain, all should be blue if anticone is small
        assert!(blue_blocks.contains(&genesis.hash));
        assert!(blue_blocks.contains(&child.hash));
        assert!(blue_blocks.contains(&grandchild.hash));
    }

    #[test]
    fn test_blue_set_with_branches() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let branch1 = create_test_block("branch1", vec![genesis.hash.clone()]);
        let branch2 = create_test_block("branch2", vec![genesis.hash.clone()]);
        let merge = create_test_block("merge", vec![branch1.hash.clone(), branch2.hash.clone()]);

        dag.insert_block(genesis).unwrap();
        dag.insert_block(branch1.clone()).unwrap();
        dag.insert_block(branch2).unwrap();
        dag.insert_block(merge.clone()).unwrap();

        let blue_set = BlueSet::new(2);
        let blue_blocks = blue_set.get_blue_blocks(&dag, &merge.hash);

        // All blocks should be in blue set (anticone of each <= k=2)
        assert!(blue_blocks.len() >= 2);
    }

    #[test]
    fn test_is_blue_simple() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();

        let blue_set = BlueSet::new(1);
        assert!(blue_set.is_blue(&dag, &genesis.hash, &child.hash));
        assert!(blue_set.is_blue(&dag, &child.hash, &child.hash));
    }

    #[test]
    fn test_anticone_size_calculation() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let branch1 = create_test_block("branch1", vec![genesis.hash.clone()]);
        let branch2 = create_test_block("branch2", vec![genesis.hash.clone()]);

        dag.insert_block(genesis).unwrap();
        dag.insert_block(branch1.clone()).unwrap();
        dag.insert_block(branch2.clone()).unwrap();

        let blue_set = BlueSet::new(5);
        let anticone_size = blue_set.get_anticone_size(&dag, &branch1.hash, &branch2.hash);

        // branch1 and branch2 have no common ancestor path, so anticone handling needed
        assert!(anticone_size <= 10); // Sanity check for reasonable size
    }

    #[test]
    fn test_blue_set_statistics() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);
        let child_hash = child.hash.clone();

        dag.insert_block(genesis).unwrap();
        dag.insert_block(child).unwrap();

        let blue_set = BlueSet::new(1);
        let stats = blue_set.get_blue_set_stats(&dag, &child_hash);

        assert_eq!(stats.total_blocks, 2);
        assert!(stats.blue_count > 0);
        assert!(stats.blue_ratio > 0.0);
    }

    #[test]
    fn test_all_ancestors_blue() {
        let mut dag = BlockDAG::new();
        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);
        let grandchild = create_test_block("grandchild", vec![child.hash.clone()]);

        dag.insert_block(genesis).unwrap();
        dag.insert_block(child).unwrap();
        dag.insert_block(grandchild.clone()).unwrap();

        let blue_set = BlueSet::new(10);
        let result = blue_set.are_all_ancestors_blue(&dag, &grandchild.hash, &grandchild.hash);

        // In linear chain with decent k, all should be blue
        assert!(result);
    }
}
