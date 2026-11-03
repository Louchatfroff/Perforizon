use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_BYTES: u64 = 256 << 20;

static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
type Slot = &'static (wgpu::PipelineCache, PathBuf);
static CACHE: Mutex<Option<Slot>> = Mutex::new(None);

fn slot() -> Option<Slot> {
    *CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub fn request(dir: Option<&Path>) {
    *DIR.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = dir.map(Path::to_path_buf);
}

fn directory() -> Option<PathBuf> {
    DIR.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub fn features(adapter: &wgpu::Adapter) -> wgpu::Features {
    if directory().is_some() && adapter.get_info().backend == wgpu::Backend::Vulkan {
        adapter.features() & wgpu::Features::PIPELINE_CACHE
    } else {
        wgpu::Features::empty()
    }
}

pub fn file_name(info: &wgpu::AdapterInfo) -> Option<String> {
    wgpu::util::pipeline_cache_key(info).map(|key| format!("{key}.bin"))
}

fn usable(data: Vec<u8>) -> Option<Vec<u8>> {
    (!data.is_empty() && data.len() as u64 <= MAX_BYTES).then_some(data)
}

pub fn init(device: &wgpu::Device, info: &wgpu::AdapterInfo) {
    *CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    if !device.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return;
    }
    let (Some(dir), Some(name)) = (directory(), file_name(info)) else {
        return;
    };
    let path = dir.join(name);
    let data = std::fs::read(&path).ok().and_then(usable);
    let loaded = data.as_ref().map(Vec::len);
    let cache = horizon_hdr::create_pipeline_cache(device, data.as_deref());
    log::info!(
        "Horizon renderer: pipeline cache {} ({})",
        path.display(),
        loaded.map_or("empty".to_owned(), |n| format!("{n} bytes loaded"))
    );
    let leaked: Slot = Box::leak(Box::new((cache, path)));
    *CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(leaked);
}

pub fn get() -> Option<&'static wgpu::PipelineCache> {
    slot().map(|(cache, _)| cache)
}

pub fn save() {
    let Some((cache, path)) = slot() else {
        return;
    };
    let Some(data) = cache.get_data().and_then(usable) else {
        return;
    };
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, &data).is_ok() && std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_outside_one_byte_to_the_cap_is_dropped() {
        assert_eq!(usable(Vec::new()), None);
        assert_eq!(usable(vec![7; 5]), Some(vec![7; 5]));
        assert!(usable(vec![0; MAX_BYTES as usize + 1]).is_none());
        assert!(usable(vec![0; MAX_BYTES as usize]).is_some());
    }

    #[test]
    fn only_vulkan_adapters_get_a_file_named_by_vendor_and_device() {
        let mut info = wgpu::AdapterInfo {
            name: String::new(),
            vendor: 0x10de,
            device: 0x2803,
            device_type: wgpu::DeviceType::DiscreteGpu,
            device_pci_bus_id: String::new(),
            driver: String::new(),
            driver_info: String::new(),
            backend: wgpu::Backend::Vulkan,
            subgroup_min_size: 0,
            subgroup_max_size: 0,
            transient_saves_memory: None,
            limit_bucket: None,
        };
        assert_eq!(
            file_name(&info).as_deref(),
            Some("wgpu_pipeline_cache_vulkan_4318_10243.bin")
        );
        info.backend = wgpu::Backend::Dx12;
        assert_eq!(file_name(&info), None);
    }
}
