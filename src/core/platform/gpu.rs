use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_video_memory: usize,
    pub is_rtx_40: bool,
}

pub fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();

    unsafe {
        let factory: Result<IDXGIFactory1, _> = CreateDXGIFactory1();
        if let Ok(factory) = factory {
            let mut index = 0;
            while let Ok(adapter) = factory.EnumAdapters1(index) {
                if let Ok(desc) = adapter.GetDesc1() {
                    let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
                    let name = String::from_utf16_lossy(&desc.Description[..len]);

                    // Ignore Microsoft Basic Render Driver
                    if desc.VendorId != 0x1414 {
                        let is_rtx_40 = is_ada_lovelace(&name, desc.VendorId, desc.DeviceId);
                        gpus.push(GpuInfo {
                            name,
                            vendor_id: desc.VendorId,
                            device_id: desc.DeviceId,
                            dedicated_video_memory: desc.DedicatedVideoMemory,
                            is_rtx_40,
                        });
                    }
                }
                index += 1;
            }
        }
    }

    gpus
}

pub fn is_ada_lovelace(name: &str, vendor_id: u32, device_id: u32) -> bool {
    if vendor_id == 0x10de {
        if (0x2600..=0x28ff).contains(&device_id) {
            return true;
        }
    }

    let upper = name.to_uppercase();
    (upper.contains("RTX 40") || upper.contains("RTX 4000"))
        && !upper.contains("RTX 4000 TURING")
}
