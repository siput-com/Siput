use super::blue_set::BlueSet;
use super::dag_traversal::DAGTraversal;
use crate::core::BlockHash;
use crate::dag::BlockDAG;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// GHOSTDAG Consensus Engine
/// Generates deterministic ordering of blocks using GHOSTDAG algorithm
#[derive(Clone, Debug)]
pub struct GHOSTDAGEngine {
    /// Parameter k: maximum anticone size for blue blocks
    pub k: usize,
    /// Reference to BlockDAG
    dag: Arc<RwLock<BlockDAG>>,
    /// Cache for blue scores to avoid recalculation
    blue_score_cache: HashMap<BlockHash, u64>,
    /// Blue set calculator
    blue_set: BlueSet,
}

impl GHOSTDAGEngine {
    /// Create new GHOSTDAG engine with parameter k
    pub fn new(k: usize) -> Self {
        GHOSTDAGEngine {
            k,
            dag: Arc::new(RwLock::new(BlockDAG::new())),
            blue_score_cache: HashMap::new(),
            blue_set: BlueSet::new(k),
        }
    }

    /// Get attached DAG
    pub fn get_dag(&self) -> Option<Arc<RwLock<BlockDAG>>> {
        Some(self.dag.clone())
    }

    /// Clear blue score cache
    pub fn clear_cache(&mut self) {
        self.blue_score_cache.clear();
    }

    /// Calculate blue score of a block (immutable version)
    fn calculate_blue_score_immutable(
        &self,
        dag: &BlockDAG,
        hash: &BlockHash,
    ) -> Result<u64, String> {
        // Check cache first
        if let Some(cached_score) = self.blue_score_cache.get(hash) {
            return Ok(*cached_score);
        }

        // Get all ancestors
        let ancestors = DAGTraversal::get_ancestors(dag, hash);

        // Count blue ancestors
        let mut blue_count = 0u64;

        // Check if block itself is blue (has anticone <= k)
        let block_anticone = DAGTraversal::get_anticone(dag, hash, hash);
        if block_anticone.len() <= self.k {
            blue_count += 1; // Count self as blue
        }

        // Count blue ancestors
        for ancestor in ancestors {
            let ancestor_anticone = DAGTraversal::get_anticone(dag, &ancestor, hash);
            if ancestor_anticone.len() <= self.k {
                blue_count += 1;
            }
        }

        Ok(blue_count)
    }

    /// Annotate a block with GhostDAG-derived metadata such as blue score and chain height.
    /// This is used to ensure blocks carry deterministic ordering metadata before being inserted into the DAG.
    pub fn annotate_block(
        &mut self,
        dag: &BlockDAG,
        block: &mut dyn crate::core::ConsensusBlock,
    ) -> Result<(), String> {
        let selected_parent = if block.parent_hashes().is_empty() {
            None
        } else {
            Some(self.select_parent_from_parents(dag, block.parent_hashes())?)
        };

        let chain_height = if let Some(parent) = selected_parent {
            dag.get_block_height(&parent).unwrap_or(0).saturating_add(1)
        } else {
            0
        };

        let blue_score = self.calculate_blue_score_for_new_block(dag, block)?;
        let topo_index = dag.block_count() as u64;

        block.apply_consensus_metadata(selected_parent, blue_score, chain_height, topo_index);

        Ok(())
    }

    /// Select best parent from a list of candidate parents using blue score and deterministic tie-breakers.
    pub fn select_parent_from_parents(
        &mut self,
        _dag: &BlockDAG,
        parents: &[BlockHash],
    ) -> Result<BlockHash, String> {
        if parents.is_empty() {
            return Err("No parent candidates provided".to_string());
        }

        let mut best_parent = parents[0];
        let mut best_score = self.get_blue_score(&best_parent).unwrap_or(0);

        for parent in parents.iter().skip(1) {
            let score = self.get_blue_score(parent).unwrap_or(0);
            if score > best_score || (score == best_score && parent < &best_parent) {
                best_parent = *parent;
                best_score = score;
            }
        }

        Ok(best_parent)
    }

    /// Calculate blue score for a block that is not yet inserted into the DAG.
    fn calculate_blue_score_for_new_block(
        &mut self,
        _dag: &BlockDAG,
        block: &dyn crate::core::ConsensusBlock,
    ) -> Result<u64, String> {
        // Basic heuristic: blue score = max parent blue score + 1
        let mut max_parent_score = 0u64;
        for parent_hash in block.parent_hashes() {
            let score = self.get_blue_score(parent_hash).unwrap_or(0);
            max_parent_score = max_parent_score.max(score);
        }
        Ok(max_parent_score.saturating_add(1))
    }

    /// Calculate blue score of a block
    /// Blue score = number of blue ancestors + 1
    pub fn calculate_blue_score(&self, hash: &BlockHash) -> Result<u64, String> {
        let dag = &*self.dag.read();

        // Check cache first
        if let Some(cached_score) = self.blue_score_cache.get(hash) {
            return Ok(*cached_score);
        }

        // Get all ancestors
        let ancestors = DAGTraversal::get_ancestors(dag, hash);

        // Count blue ancestors
        let mut blue_count = 0u64;

        // Check if block itself is blue (has anticone <= k)
        let block_anticone = DAGTraversal::get_anticone(dag, hash, hash);
        if block_anticone.len() <= self.k {
            blue_count += 1; // Count self as blue
        }

        // Count blue ancestors
        for ancestor in ancestors {
            let ancestor_anticone = DAGTraversal::get_anticone(dag, &ancestor, hash);
            if ancestor_anticone.len() <= self.k {
                blue_count += 1;
            }
        }

        // This implementation is intentionally read-only for manager-friendly usage.
        // Cache mutating operations can be implemented later with interior mutability.

        Ok(blue_count)
    }

    /// Get blue score from cache or calculate
    pub fn get_blue_score(&self, hash: &BlockHash) -> Result<u64, String> {
        if let Some(cached) = self.blue_score_cache.get(hash) {
            return Ok(*cached);
        }

        // Fallback to block header blue_score if available
        if let Some(block) = self.dag.read().get_block(hash) {
            return Ok(block.header.blue_score);
        }

        Err(format!("Blue score for block {} unavailable", hex::encode(hash)))
    }

    /// Select best parent based on blue score
    /// Returns parent with highest blue score, ties broken by hash ordering
    pub fn select_parent(&mut self, block_hash: &BlockHash) -> Result<BlockHash, String> {
        let dag = &*self.dag.read();

        let parents = DAGTraversal::get_parents(dag, block_hash);

        if parents.is_empty() {
            return Err("Block has no parents".to_string());
        }

        if parents.len() == 1 {
            return Ok(parents[0].clone());
        }

        // Score each parent
        let mut parent_scores: Vec<(BlockHash, u64)> = Vec::new();

        for parent in parents {
            let score = self.calculate_blue_score(&parent)?;
            parent_scores.push((parent, score));
        }

        // Sort by score (descending), then by hash (ascending) for determinism
        parent_scores.sort_by(|a, b| {
            match b.1.cmp(&a.1) {
                std::cmp::Ordering::Equal => a.0.cmp(&b.0), // Deterministic tiebreaker
                other => other,
            }
        });

        Ok(parent_scores[0].0.clone())
    }

    /// Generate topological ordering using GHOSTDAG
    pub fn generate_ordering(&mut self) -> Result<Vec<BlockHash>, String> {
        let dag = &*self.dag.read();

        // First, collect all blocks and calculate their scores
        let all_blocks = DAGTraversal::get_all_blocks(dag);
        let mut block_scores = HashMap::new();

        for block_hash in &all_blocks {
            let score = self.calculate_blue_score_immutable(dag, block_hash)?;
            block_scores.insert(block_hash.clone(), score);
        }

        // Now use the scores for ordering
        let mut ordering = Vec::new();

        if all_blocks.is_empty() {
            return Ok(ordering);
        }

        // Start with genesis blocks (no parents)
        let mut genesis_blocks: Vec<_> = all_blocks
            .iter()
            .filter(|h| {
                dag.get_block(h)
                    .map(|b| b.header.parent_hashes.is_empty())
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        genesis_blocks.sort(); // Deterministic order
        ordering.extend(genesis_blocks);

        // Process remaining blocks
        let mut processed = ordering.iter().cloned().collect::<HashSet<_>>();

        while processed.len() < all_blocks.len() {
            let mut next_blocks = Vec::new();

            for block_hash in &all_blocks {
                if processed.contains(block_hash) {
                    continue;
                }

                // Check if all parents are processed
                let parents = DAGTraversal::get_parents(dag, block_hash);
                if parents.iter().all(|p| processed.contains(p)) {
                    next_blocks.push(block_hash.clone());
                }
            }

            if next_blocks.is_empty() {
                break; // No more blocks can be processed (shouldn't happen in valid DAG)
            }

            // Score blocks and sort
            let mut scored_blocks: Vec<(BlockHash, u64)> = Vec::new();

            for block in next_blocks {
                let score = block_scores.get(&block).copied().unwrap_or(0);
                scored_blocks.push((block, score));
            }

            // Sort by blue score (descending), then hash (ascending)
            scored_blocks.sort_by(|a, b| match b.1.cmp(&a.1) {
                std::cmp::Ordering::Equal => a.0.cmp(&b.0),
                other => other,
            });

            for (block, _) in scored_blocks {
                ordering.push(block.clone());
                processed.insert(block);
            }
        }

        Ok(ordering)
    }

    /// Validate ordering
    pub fn validate_ordering(&self, ordering: &[BlockHash]) -> Result<(), String> {
        let dag = &*self.dag.read();

        let mut seen = HashSet::new();

        for (index, block_hash) in ordering.iter().enumerate() {
            // Check block exists
            if !dag.get_block(block_hash).is_some() {
                return Err(format!(
                    "Block {} not found in DAG",
                    hex::encode(block_hash)
                ));
            }

            // Check no duplicates
            if seen.contains(block_hash) {
                return Err(format!(
                    "Duplicate block {} at position {}",
                    hex::encode(block_hash),
                    index
                ));
            }
            seen.insert(block_hash.clone());

            // Check all parents appear before child
            let parents = DAGTraversal::get_parents(dag, block_hash);
            for parent in parents {
                let parent_pos = ordering.iter().position(|h| h == &parent);
                if parent_pos.is_none() {
                    return Err(format!(
                        "Parent {} of block {} not in ordering",
                        hex::encode(parent),
                        hex::encode(block_hash)
                    ));
                }

                if parent_pos.unwrap() >= index {
                    return Err(format!(
                        "Parent {} appears after child {} in ordering",
                        hex::encode(parent),
                        hex::encode(block_hash)
                    ));
                }
            }
        }

        // Check all blocks are included
        if seen.len() != dag.block_count() {
            return Err(format!(
                "Ordering has {} blocks but DAG has {} blocks",
                seen.len(),
                dag.block_count()
            ));
        }

        Ok(())
    }

    /// Get GHOSTDAG statistics
    pub fn get_stats(&self) -> GHOSTDAGStats {
        let dag = &*self.dag.read();
        let blue_set = self.blue_set.build_blue_set(dag, &dag.get_tips()[0]);
        let red_set = self.blue_set.build_red_set(dag, &dag.get_tips()[0]);

        GHOSTDAGStats {
            k_parameter: self.k,
            total_blocks: dag.block_count(),
            blue_blocks: blue_set.len(),
            red_blocks: red_set.len(),
            cache_size: self.blue_score_cache.len(),
        }
    }

    /// Get blue score for all blocks
    pub fn get_all_blue_scores(&self) -> Result<HashMap<BlockHash, u64>, String> {
        let dag = &*self.dag.read();

        let mut scores = HashMap::new();
        let all_blocks = DAGTraversal::get_all_blocks(dag);

        for block_hash in all_blocks {
            let score = self.calculate_blue_score_immutable(dag, &block_hash)?;
            scores.insert(block_hash, score);
        }

        Ok(scores)
    }

    /// Get blue blocks for reference
    pub fn get_blue_blocks(&self, reference_hash: &BlockHash) -> Result<Vec<BlockHash>, String> {
        let dag = &*self.dag.read();
        Ok(self.blue_set.get_blue_blocks(dag, reference_hash))
    }

    /// Get red blocks for reference
    pub fn get_red_blocks(&self, reference_hash: &BlockHash) -> Result<Vec<BlockHash>, String> {
        let dag = &*self.dag.read();
        Ok(self.blue_set.get_red_blocks(dag, reference_hash))
    }
}

/// GHOSTDAG statistics
#[derive(Clone, Debug)]
pub struct GHOSTDAGStats {
    pub k_parameter: usize,
    pub total_blocks: usize,
    pub blue_blocks: usize,
    pub red_blocks: usize,
    pub cache_size: usize,
}

/// Configuration for GHOSTDAG consensus
#[derive(Clone, Debug)]
pub struct GhostDagConfig {
    pub k: usize,
    pub max_merge_depth: usize,
    pub safe_reorg_depth: usize,
    pub finality_threshold: u64,
    pub max_dag_density: f64,
    pub cache_size: usize,
}

impl Default for GhostDagConfig {
    fn default() -> Self {
        Self {
            k: 2,
            max_merge_depth: 100,
            safe_reorg_depth: 10,
            finality_threshold: 100,
            max_dag_density: 0.5,
            cache_size: 10000,
        }
    }
}

/// Result of GHOSTDAG operations
#[derive(Clone, Debug)]
pub enum GhostDagResult<T> {
    Ok(T),
    Err(String),
}

/// Blue work structure for block ordering
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlueWork {
    pub blue_score: u64,
    pub cumulative_work: u64,
    pub timestamp: u64,
}

impl PartialOrd for BlueWork {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BlueWork {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher blue_score first
        match other.blue_score.cmp(&self.blue_score) {
            std::cmp::Ordering::Equal => {
                // Higher cumulative_work first
                match other.cumulative_work.cmp(&self.cumulative_work) {
                    std::cmp::Ordering::Equal => {
                        // Older timestamp first
                        self.timestamp.cmp(&other.timestamp)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        }
    }
}

/// Main GHOSTDAG manager with concurrency support
pub struct GhostDagManager {
    config: GhostDagConfig,
    dag: Arc<RwLock<BlockDAG>>,
    blue_score_cache: Arc<RwLock<HashMap<BlockHash, u64>>>,
    past_set_cache: Arc<RwLock<HashMap<BlockHash, HashSet<BlockHash>>>>,
    future_set_cache: Arc<RwLock<HashMap<BlockHash, HashSet<BlockHash>>>>,
    merge_set_cache: Arc<RwLock<HashMap<(BlockHash, BlockHash), HashSet<BlockHash>>>>,
    pruning_point: Arc<RwLock<Option<BlockHash>>>,
}

impl GhostDagManager {
    /// Create new GhostDagManager with config
    pub fn new(config: GhostDagConfig, dag: Arc<RwLock<BlockDAG>>) -> Self {
        GhostDagManager {
            config,
            dag,
            blue_score_cache: Arc::new(RwLock::new(HashMap::new())),
            past_set_cache: Arc::new(RwLock::new(HashMap::new())),
            future_set_cache: Arc::new(RwLock::new(HashMap::new())),
            merge_set_cache: Arc::new(RwLock::new(HashMap::new())),
            pruning_point: Arc::new(RwLock::new(None)),
        }
    }

    /// Process a new block and update consensus state
    pub async fn process_block(&self, block: &crate::Block) -> GhostDagResult<()> {
        // Validate block
        if let Err(e) = self.validate_block(block).await {
            return GhostDagResult::Err(e);
        }

        // Insert into DAG
        {
            let mut dag = self.dag.write();
            if let Err(e) = dag.insert_block(block.clone()) {
                return GhostDagResult::Err(format!("Failed to insert block: {}", e));
            }
        }

        // Update caches
        self.update_caches(&block.hash).await;

        // Check for attacks
        if let Err(e) = self.detect_attack_patterns(&block.hash).await {
            return GhostDagResult::Err(e);
        }

        // Update pruning point if needed
        self.update_pruning_point().await;

        GhostDagResult::Ok(())
    }

    /// Validate block before processing
    async fn validate_block(&self, block: &crate::Block) -> Result<(), String> {
        let dag = self.dag.read();

        // Check parents exist
        for parent in &block.header.parent_hashes {
            if !dag.get_block(parent).is_some() {
                return Err(format!("Parent block {} not found", hex::encode(parent)));
            }
        }

        // Check merge depth
        let merge_depth = self
            .compute_merge_depth(&block.header.parent_hashes)
            .await?;
        if merge_depth > self.config.max_merge_depth {
            return Err(format!(
                "Merge depth {} exceeds max {}",
                merge_depth, self.config.max_merge_depth
            ));
        }

        // Check DAG density
        let density = self.compute_dag_density().await?;
        if density > self.config.max_dag_density {
            return Err(format!(
                "DAG density {} exceeds max {}",
                density, self.config.max_dag_density
            ));
        }

        Ok(())
    }

    /// Compute merge depth for parents
    async fn compute_merge_depth(&self, parents: &[BlockHash]) -> Result<usize, String> {
        if parents.len() <= 1 {
            return Ok(0);
        }

        let mut max_depth = 0;
        for i in 0..parents.len() {
            for j in (i + 1)..parents.len() {
                let merge_set = match self.compute_merge_set(&parents[i], &parents[j]).await {
                    GhostDagResult::Ok(set) => set,
                    GhostDagResult::Err(e) => return Err(e),
                };
                max_depth = max_depth.max(merge_set.len());
            }
        }

        Ok(max_depth)
    }

    /// Compute DAG density heuristic
    async fn compute_dag_density(&self) -> Result<f64, String> {
        let dag = self.dag.read();
        let total_blocks = dag.block_count();
        if total_blocks == 0 {
            return Ok(0.0);
        }

        let tips = dag.get_tips();
        let mut total_edges = 0;
        for tip in tips {
            let parents = dag
                .get_block(&tip)
                .map(|b| b.header.parent_hashes.len())
                .unwrap_or(0);
            total_edges += parents;
        }

        Ok(total_edges as f64 / total_blocks as f64)
    }

    /// Update caches after block insertion
    async fn update_caches(&self, block_hash: &BlockHash) {
        // Clear affected caches (simplified)
        let mut blue_cache = self.blue_score_cache.write();
        blue_cache.remove(block_hash);

        let mut past_cache = self.past_set_cache.write();
        past_cache.remove(block_hash);

        let mut future_cache = self.future_set_cache.write();
        future_cache.remove(block_hash);

        // Note: merge_set_cache would need more complex invalidation
    }

    /// Select blue set for a block
    pub async fn select_blue_set(
        &self,
        block_hash: &BlockHash,
    ) -> GhostDagResult<HashSet<BlockHash>> {
        let dag = self.dag.read();
        let blue_set = BlueSet::new(self.config.k);
        let blue_blocks = blue_set.build_blue_set(&dag, block_hash);
        GhostDagResult::Ok(blue_blocks)
    }

    /// Compute blue score for a block
    pub async fn compute_blue_score(&self, block_hash: &BlockHash) -> GhostDagResult<u64> {
        // Check cache first
        {
            let cache = self.blue_score_cache.read();
            if let Some(score) = cache.get(block_hash) {
                return GhostDagResult::Ok(*score);
            }
        }

        let dag = self.dag.read();
        let ancestors = DAGTraversal::get_ancestors(&dag, block_hash);

        let mut blue_count = 0u64;
        // Check self
        let anticone = DAGTraversal::get_anticone(&dag, block_hash, block_hash);
        if anticone.len() <= self.config.k {
            blue_count += 1;
        }

        // Check ancestors
        for ancestor in ancestors {
            let anticone = DAGTraversal::get_anticone(&dag, &ancestor, block_hash);
            if anticone.len() <= self.config.k {
                blue_count += 1;
            }
        }

        // Cache result
        {
            let mut cache = self.blue_score_cache.write();
            cache.insert(block_hash.clone(), blue_count);
        }

        GhostDagResult::Ok(blue_count)
    }

    /// Compute merge set between two blocks
    pub async fn compute_merge_set(
        &self,
        block_a: &BlockHash,
        block_b: &BlockHash,
    ) -> GhostDagResult<HashSet<BlockHash>> {
        let key = if block_a < block_b {
            (*block_a, *block_b)
        } else {
            (*block_b, *block_a)
        };

        // Check cache
        {
            let cache = self.merge_set_cache.read();
            if let Some(set) = cache.get(&key) {
                return GhostDagResult::Ok(set.clone());
            }
        }

        let _dag = self.dag.read();
        let past_a = match self.get_past_set(block_a).await {
            GhostDagResult::Ok(set) => set,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };
        let past_b = match self.get_past_set(block_b).await {
            GhostDagResult::Ok(set) => set,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let merge_set: HashSet<BlockHash> = past_a.intersection(&past_b).cloned().collect();

        // Cache result
        {
            let mut cache = self.merge_set_cache.write();
            cache.insert(key, merge_set.clone());
        }

        GhostDagResult::Ok(merge_set)
    }

    /// Get past set (ancestors) with caching
    async fn get_past_set(&self, block_hash: &BlockHash) -> GhostDagResult<HashSet<BlockHash>> {
        // Check cache
        {
            let cache = self.past_set_cache.read();
            if let Some(set) = cache.get(block_hash) {
                return GhostDagResult::Ok(set.clone());
            }
        }

        let dag = self.dag.read();
        let past_set = DAGTraversal::get_ancestors(&dag, block_hash);

        // Cache result
        {
            let mut cache = self.past_set_cache.write();
            cache.insert(block_hash.clone(), past_set.clone());
        }

        GhostDagResult::Ok(past_set)
    }

    /// Compute virtual selected parent from tips
    pub async fn compute_virtual_selected_parent(
        &self,
        tips: &[BlockHash],
    ) -> GhostDagResult<BlockHash> {
        if tips.is_empty() {
            return GhostDagResult::Err("No tips provided".to_string());
        }

        if tips.len() == 1 {
            return GhostDagResult::Ok(tips[0].clone());
        }

        // Find tip with highest blue score
        let mut best_tip = tips[0];
        let best_score = match self.compute_blue_score(&best_tip).await {
            GhostDagResult::Ok(score) => score,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let mut best_score = best_score;
        for tip in tips.iter().skip(1) {
            let score = match self.compute_blue_score(tip).await {
                GhostDagResult::Ok(s) => s,
                GhostDagResult::Err(e) => return GhostDagResult::Err(e),
            };
            if score > best_score {
                best_score = score;
                best_tip = *tip;
            } else if score == best_score {
                // Tie break by hash
                if tip < &best_tip {
                    best_tip = *tip;
                }
            }
        }

        GhostDagResult::Ok(best_tip)
    }

    /// Check if block is final
    pub async fn is_final(&self, block_hash: &BlockHash) -> GhostDagResult<bool> {
        let current_blue_score = match self.compute_blue_score(block_hash).await {
            GhostDagResult::Ok(score) => score,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let tips = match self.get_tips().await {
            GhostDagResult::Ok(t) => t,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let virtual_parent = match self.compute_virtual_selected_parent(&tips).await {
            GhostDagResult::Ok(parent) => parent,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let virtual_blue_score = match self.compute_blue_score(&virtual_parent).await {
            GhostDagResult::Ok(score) => score,
            GhostDagResult::Err(e) => return GhostDagResult::Err(e),
        };

        let distance = virtual_blue_score.saturating_sub(current_blue_score);
        GhostDagResult::Ok(distance > self.config.finality_threshold)
    }

    /// Get current tips
    async fn get_tips(&self) -> GhostDagResult<Vec<BlockHash>> {
        let dag = self.dag.read();
        GhostDagResult::Ok(dag.get_tips())
    }

    /// Detect attack patterns
    pub async fn detect_attack_patterns(&self, block_hash: &BlockHash) -> Result<(), String> {
        let blue_score = match self.compute_blue_score(block_hash).await {
            GhostDagResult::Ok(score) => score,
            GhostDagResult::Err(e) => return Err(e),
        };

        // Check for selfish mining (sudden blue score jump)
        let parents = {
            let dag = self.dag.read();
            dag.get_block(block_hash)
                .map(|b| b.header.parent_hashes.clone())
                .unwrap_or_default()
        };

        for parent in parents {
            let parent_score = match self.compute_blue_score(&parent).await {
                GhostDagResult::Ok(score) => score,
                GhostDagResult::Err(e) => return Err(e),
            };
            if blue_score > parent_score + 10 {
                // Arbitrary threshold
                return Err("Detected selfish mining attack".to_string());
            }
        }

        // Check for parasite chain
        let tips = match self.get_tips().await {
            GhostDagResult::Ok(t) => t,
            GhostDagResult::Err(e) => return Err(e),
        };
        let virtual_parent = match self.compute_virtual_selected_parent(&tips).await {
            GhostDagResult::Ok(parent) => parent,
            GhostDagResult::Err(e) => return Err(e),
        };
        let merge_set = match self.compute_merge_set(block_hash, &virtual_parent).await {
            GhostDagResult::Ok(set) => set,
            GhostDagResult::Err(e) => return Err(e),
        };
        if merge_set.len() < 5 {
            // Arbitrary threshold
            return Err("Detected parasite chain attack".to_string());
        }

        Ok(())
    }

    /// Update pruning point
    pub async fn update_pruning_point(&self) {
        let tips = match self.get_tips().await {
            GhostDagResult::Ok(t) => t,
            GhostDagResult::Err(_) => return,
        };
        if tips.is_empty() {
            return;
        }

        let virtual_parent = match self.compute_virtual_selected_parent(&tips).await {
            GhostDagResult::Ok(parent) => parent,
            GhostDagResult::Err(_) => tips[0],
        };
        let blue_score = match self.compute_blue_score(&virtual_parent).await {
            GhostDagResult::Ok(score) => score,
            GhostDagResult::Err(_) => 0,
        };

        // Find block with blue_score <= current - safe_reorg_depth
        let dag = self.dag.read();
        let all_blocks = DAGTraversal::get_all_blocks(&dag);

        let mut candidate = None;
        for block in all_blocks {
            let score = match self.compute_blue_score(&block).await {
                GhostDagResult::Ok(s) => s,
                GhostDagResult::Err(_) => 0,
            };
            if blue_score.saturating_sub(score) >= self.config.safe_reorg_depth as u64 {
                if candidate.is_none() || &block < candidate.as_ref().unwrap() {
                    candidate = Some(block);
                }
            }
        }

        if let Some(pp) = candidate {
            *self.pruning_point.write() = Some(pp);
        }
    }

    /// Get current pruning point
    pub fn get_pruning_point(&self) -> Option<BlockHash> {
        *self.pruning_point.read()
    }

    /// Replace or update the underlying DAG state for this manager.
    pub fn attach_dag(&mut self, dag: BlockDAG) {
        let mut locked = self.dag.write();
        *locked = dag;
    }

    /// Get virtual selected parent (simplified: return first tip)
    pub fn get_virtual_selected_parent(&self) -> BlockHash {
        self.dag.read().get_tips().first().cloned().unwrap_or([0u8; 32])
    }

    /// Get virtual blue score (simplified: return blue score of first tip)
    pub fn get_virtual_blue_score(&self) -> u64 {
        let tips = self.dag.read().get_tips();
        if let Some(tip) = tips.first() {
            self.get_blue_score(tip).unwrap_or(0)
        } else {
            0
        }
    }

    /// Get blue score from the DAG model
    pub fn get_blue_score(&self, hash: &BlockHash) -> Result<u64, String> {
        if let Some(block) = self.dag.read().get_block(hash) {
            Ok(block.header.blue_score)
        } else {
            Err(format!("Blue score for block {} unavailable", hex::encode(hash)))
        }
    }

    /// Annotate an individual block using current DAG tip state.
    pub fn annotate_block(&self, _dag: &BlockDAG, _block: &mut dyn crate::core::ConsensusBlock) -> Result<(), String> {
        // Simplified stub: preserve existing metadata and layout.
        Ok(())
    }

    /// Generate deterministic ordering for the DAG; used by mining/high-level core consensus.
    pub fn generate_ordering(&self) -> Result<Vec<BlockHash>, String> {
        let dag = self.dag.read();
        let mut ordering = dag.get_topological_order();
        ordering.sort();
        Ok(ordering)
    }
}

impl crate::consensus::ConsensusEngine for GHOSTDAGEngine {
    fn attach_dag(&mut self, dag: crate::dag::blockdag::BlockDAG) {
        let mut locked = self.dag.write();
        *locked = dag;
    }

    fn get_virtual_selected_parent(&self) -> BlockHash {
        self.dag.read().get_tips().first().cloned().unwrap_or([0u8; 32])
    }

    fn get_virtual_blue_score(&self) -> u64 {
        let tips = self.dag.read().get_tips();
        if let Some(tip) = tips.first() {
            self.get_blue_score(tip).unwrap_or(0)
        } else {
            0
        }
    }

    fn calculate_blue_score(&self, hash: &BlockHash) -> Result<u64, String> {
        self.get_blue_score(hash)
    }

    fn annotate_block(&mut self, dag: &BlockDAG, block: &mut dyn crate::core::ConsensusBlock) -> Result<(), String> {
        self.annotate_block(dag, block)
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
    fn test_ghostdag_engine_creation() {
        let engine = GHOSTDAGEngine::new(3);
        assert_eq!(engine.k, 3);
        assert_eq!(engine.blue_score_cache.len(), 0);
    }

    #[test]
    fn test_attach_dag() {
        let mut engine = GHOSTDAGEngine::new(3);
        let dag = BlockDAG::new();
        engine.attach_dag(dag);
        assert!(engine.get_dag().is_some());
    }

    #[test]
    fn test_blue_score_calculation() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();

        engine.attach_dag(dag);

        let genesis_score = engine.calculate_blue_score(&genesis.hash).unwrap();
        let child_score = engine.calculate_blue_score(&child.hash).unwrap();

        assert!(genesis_score > 0);
        assert!(child_score > 0);
    }

    #[test]
    fn test_select_parent() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        // Create genesis
        dag.create_genesis_if_empty();
        let genesis_hash = dag.get_tips()[0].clone();

        // Create parents from genesis
        let parent1 = create_test_block("parent1", vec![genesis_hash.clone()]);
        let parent2 = create_test_block("parent2", vec![genesis_hash.clone()]);
        let child = create_test_block("child", vec![parent1.hash.clone(), parent2.hash.clone()]);

        dag.insert_block(parent1.clone()).unwrap();
        dag.insert_block(parent2.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();

        engine.attach_dag(dag);

        let best_parent = engine.select_parent(&child.hash).unwrap();
        assert!(best_parent == parent1.hash || best_parent == parent2.hash);
    }

    #[test]
    fn test_generate_ordering() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        let child1 = create_test_block("child1", vec![genesis.hash.clone()]);
        let child2 = create_test_block("child2", vec![genesis.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child1.clone()).unwrap();
        dag.insert_block(child2.clone()).unwrap();

        engine.attach_dag(dag);

        let ordering = engine.generate_ordering().unwrap();
        assert_eq!(ordering.len(), 3);

        // Genesis should come first
        assert_eq!(ordering[0], genesis.hash);
    }

    #[test]
    fn test_validate_ordering() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();

        engine.attach_dag(dag);

        let ordering = vec![genesis.hash.clone(), child.hash.clone()];
        let result = engine.validate_ordering(&ordering);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_ordering_invalid() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);

        dag.insert_block(genesis.clone()).unwrap();
        dag.insert_block(child.clone()).unwrap();

        engine.attach_dag(dag);

        // Invalid: child before parent
        let invalid_ordering = vec![child.hash.clone(), genesis.hash.clone()];
        let result = engine.validate_ordering(&invalid_ordering);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_stats() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        dag.insert_block(genesis).unwrap();

        engine.attach_dag(dag);

        let stats = engine.get_stats();
        assert_eq!(stats.k_parameter, 2);
        assert!(stats.total_blocks > 0);
    }

    #[test]
    fn test_cache_effectiveness() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        dag.insert_block(genesis.clone()).unwrap();

        engine.attach_dag(dag);

        // Calculate same block twice
        let _ = engine.calculate_blue_score(&genesis.hash);
        let _ = engine.get_blue_score(&genesis.hash);

        assert_eq!(engine.blue_score_cache.len(), 1);
    }

    #[test]
    fn test_all_blue_scores() {
        let mut engine = GHOSTDAGEngine::new(2);
        let mut dag = BlockDAG::new();

        let genesis = create_test_block("genesis", vec![]);
        let child = create_test_block("child", vec![genesis.hash.clone()]);

        dag.insert_block(genesis).unwrap();
        dag.insert_block(child).unwrap();

        engine.attach_dag(dag);

        let scores = engine.get_all_blue_scores().unwrap();
        assert_eq!(scores.len(), 2);
    }
}
