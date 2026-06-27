use crate::config;
use crate::tools::types::*;
use crate::types::*;

/// List all configured pools
pub async fn pools_list() -> Result<PoolListOutput, String> {
    match config::load_pools().await {
        Ok(pf) => Ok(PoolListOutput { pools: pf.pools }),
        Err(e) => Err(format!("Failed to load pools: {}", e)),
    }
}

/// Create or update a pool
pub async fn pools_upsert(input: PoolUpsertInput) -> Result<PoolUpsertOutput, String> {
    match config::load_pools().await {
        Ok(mut pf) => {
            if let Some(idx) = pf.pools.iter().position(|p| p.id == input.id) {
                pf.pools[idx] = Pool {
                    id: input.id.clone(),
                    name: input.name,
                    description: input.description,
                    is_active: input.is_active,
                };
            } else {
                pf.pools.push(Pool {
                    id: input.id,
                    name: input.name,
                    description: input.description,
                    is_active: input.is_active,
                });
            }
            config::save_pools(&pf)
                .await
                .map_err(|e| format!("Save failed: {}", e))?;
            Ok(PoolUpsertOutput { updated: true })
        }
        Err(e) => Err(format!("Failed to load pools: {}", e)),
    }
}

/// Delete a pool by id
pub async fn pools_delete(input: PoolDeleteInput) -> Result<PoolDeleteOutput, String> {
    match config::load_pools().await {
        Ok(mut pf) => {
            let before = pf.pools.len();
            pf.pools.retain(|p| p.id != input.id);
            let removed = pf.pools.len() < before;
            config::save_pools(&pf)
                .await
                .map_err(|e| format!("Save failed: {}", e))?;
            Ok(PoolDeleteOutput { removed })
        }
        Err(e) => Err(format!("Failed to load pools: {}", e)),
    }
}
