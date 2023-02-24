// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// Disable warnings for unused engine_rocks's feature.
#![allow(dead_code)]
#![allow(unused_variables)]
use std::{
    fmt::Debug,
    fs,
    path::Path,
    sync::{atomic::AtomicIsize, Arc},
};

use engine_rocks::RocksSnapshot;
use engine_traits::{Checkpointable, Checkpointer, Error, KvEngine, Result};
use rocksdb::DB;

use crate::{
    proxy_utils::{engine_ext::*, EngineStoreHub},
    ProxyEngineExt,
};

#[derive(Clone, Debug)]
pub struct RocksEngine {
    // Must ensure rocks is the first field, for RocksEngine::from_ref.
    // We must own a engine_rocks::RocksEngine, since TiKV has not decouple from engine_rocks yet.
    pub rocks: engine_rocks::RocksEngine,
    pub proxy_ext: ProxyEngineExt,
    pub ps_ext: Option<PageStorageExt>,
}

impl RocksEngine {
    pub(crate) fn new(db: DB) -> RocksEngine {
        RocksEngine::from_db(Arc::new(db))
    }

    pub fn init(
        &mut self,
        engine_store_server_helper: isize,
        snap_handle_pool_size: usize,
        engine_store_hub: Option<Arc<dyn EngineStoreHub + Send + Sync>>,
        config_set: Option<Arc<crate::ProxyEngineConfigSet>>,
    ) {
        #[cfg(feature = "enable-pagestorage")]
        tikv_util::info!("enabled pagestorage");
        #[cfg(not(feature = "enable-pagestorage"))]
        tikv_util::info!("disabled pagestorage");
        self.proxy_ext = ProxyEngineExt {
            engine_store_server_helper,
            pool_capacity: snap_handle_pool_size,
            pending_applies_count: Arc::new(AtomicIsize::new(0)),
            engine_store_hub,
            config_set,
            cached_region_info_manager: Some(Arc::new(crate::CachedRegionInfoManager::new())),
        };
        self.ps_ext = Some(PageStorageExt {
            engine_store_server_helper,
        });
    }

    pub fn from_rocks(rocks: engine_rocks::RocksEngine) -> Self {
        RocksEngine {
            rocks,
            proxy_ext: ProxyEngineExt::default(),
            ps_ext: None,
        }
    }

    pub fn from_db(db: Arc<DB>) -> Self {
        RocksEngine {
            rocks: engine_rocks::RocksEngine::from_db(db),
            proxy_ext: ProxyEngineExt::default(),
            ps_ext: None,
        }
    }

    pub fn from_ref(db: &Arc<DB>) -> &Self {
        unsafe { &*(db as *const Arc<DB> as *const RocksEngine) }
    }

    pub fn as_inner(&self) -> &Arc<DB> {
        self.rocks.as_inner()
    }

    pub fn get_sync_db(&self) -> Arc<DB> {
        self.rocks.get_sync_db()
    }

    pub fn exists(path: &str) -> bool {
        let path = Path::new(path);
        if !path.exists() || !path.is_dir() {
            return false;
        }

        // If path is not an empty directory, we say db exists. If path is not an empty
        // directory but db has not been created, `DB::list_column_families`
        // fails and we can clean up the directory by this indication.
        fs::read_dir(&path).unwrap().next().is_some()
    }

    pub fn support_multi_batch_write(&self) -> bool {
        self.rocks.support_multi_batch_write()
    }
}

impl KvEngine for RocksEngine {
    type Snapshot = RocksSnapshot;

    fn snapshot(&self) -> RocksSnapshot {
        self.rocks.snapshot()
    }

    fn sync(&self) -> Result<()> {
        self.rocks.sync()
    }

    fn flush_metrics(&self, instance: &str) {
        self.rocks.flush_metrics(instance);
    }

    fn bad_downcast<T: 'static>(&self) -> &T {
        self.rocks.bad_downcast()
    }

    fn can_apply_snapshot(&self, is_timeout: bool, new_batch: bool, region_id: u64) -> bool {
        self.proxy_ext
            .can_apply_snapshot(is_timeout, new_batch, region_id)
    }
}

impl RocksEngine {
    pub fn do_write(&self, cf: &str, key: &[u8]) -> bool {
        crate::do_write(cf, key)
    }
}

pub struct TiFlashCheckpointer {}

impl Checkpointable for RocksEngine {
    type Checkpointer = TiFlashCheckpointer;

    fn new_checkpointer(&self) -> Result<Self::Checkpointer> {
        Err(Error::Other("TiFlash don't support Checkpointable".into()))
    }
}

impl Checkpointer for TiFlashCheckpointer {
    fn create_at(
        &mut self,
        db_out_dir: &Path,
        titan_out_dir: Option<&Path>,
        log_size_for_flush: u64,
    ) -> Result<()> {
        Err(Error::Other(
            "TiFlash don't support Checkpointer::create_at".into(),
        ))
    }
}