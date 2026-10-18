//! Bounded immutable preparation on the same pool, never a second pool or a
//! concurrent Work traversal. Indexed collection preserves source order.
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

impl CandidateRoutedWorkerPool {
    /// Map an already bounded batch of immutable tasks in input order.
    /// Result storage is reserved before any operation runs. Callers can map
    /// to Result<T,E> and choose the first input-order failure after all jobs
    /// join. Each operation must honor the caller's existing memory/cancel
    /// policy; no new thread pool or global scheduler is constructed.
    pub fn map_ordered<T, R, F>(
        &self,
        input: Vec<T>,
        operation: F,
    ) -> Result<Vec<R>, super::super::super::CandidateRoutedError>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Send + Sync,
    {
        let _lease = self.acquire(self.workers)?;
        let mut output = Vec::new();
        output.try_reserve_exact(input.len()).map_err(|error| {
            super::super::super::CandidateRoutedError::InvalidInput(format!(
                "cannot reserve ordered preparation results: {error}"
            ))
        })?;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pool) = &self.pool {
            pool.install(|| {
                input
                    .into_par_iter()
                    .map(operation)
                    .collect_into_vec(&mut output)
            });
            return Ok(output);
        }
        output.extend(input.into_iter().map(operation));
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_pool_preparation_is_stable_across_widths_and_rejects_reentry() {
        for workers in [1, 4] {
            if !crate::test_gates::workers_or_skip("ordered pool preparation", workers) {
                continue;
            }
            let pool = CandidateRoutedWorkerPool::try_new(workers).unwrap();
            let result = pool
                .map_ordered((0..32).collect(), |ordinal| {
                    if ordinal % 3 == 0 {
                        std::thread::yield_now();
                    }
                    assert!(pool.map_ordered(vec![0], |n| n).is_err());
                    if ordinal == 3 || ordinal == 17 {
                        Err(ordinal)
                    } else {
                        Ok(ordinal * ordinal)
                    }
                })
                .unwrap();
            assert_eq!(result.iter().position(Result::is_err), Some(3));
            assert_eq!(result[31], Ok(31 * 31));
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                pool.map_ordered(vec![0, 1, 2], |i| {
                    if i == 1 {
                        panic!("preparation worker panic");
                    }
                    i
                })
            }));
            assert!(panic.is_err());
            assert_eq!(pool.map_ordered(vec![2, 3], |n| n + 1).unwrap(), vec![3, 4]);
        }
    }
}
