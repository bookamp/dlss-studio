use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

/// RAII Temporary Directory for test isolation.
/// Automatically cleans up on Drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let name = format!("dlss_test_{}_{}_{}_{}", prefix, pid, count, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros());
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&path).expect("Failed to create temp test directory");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }

    pub fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let p = self.path.join(path);
        std::fs::create_dir_all(&p).expect("Failed to create nested test dir");
        p
    }

    pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(&self, path: P, contents: C) -> PathBuf {
        let p = self.path.join(path);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&p, contents).expect("Failed to write test file");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Creates minimal valid 64-bit PE buffer for tests that require PE header parsing.
pub fn create_minimal_pe64_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; 1024];
    // DOS header: 'MZ' at 0
    bytes[0] = b'M';
    bytes[1] = b'Z';
    // e_lfanew at 0x3C -> 0x80
    bytes[0x3C] = 0x80;
    bytes[0x3D] = 0x00;
    bytes[0x3E] = 0x00;
    bytes[0x3F] = 0x00;

    // PE signature at 0x80 -> 'PE\0\0'
    bytes[0x80] = b'P';
    bytes[0x81] = b'E';
    bytes[0x82] = 0;
    bytes[0x83] = 0;

    // Machine = IMAGE_FILE_MACHINE_AMD64 (0x8664) at 0x84
    bytes[0x84] = 0x64;
    bytes[0x85] = 0x86;

    // Magic = PE32+ (0x020B) at 0x98 (OptionalHeader offset 0x18 from PE signature 0x80)
    bytes[0x98] = 0x0B;
    bytes[0x99] = 0x02;

    bytes
}
