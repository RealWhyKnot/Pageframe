use peniko::{Blob, WeakBlob};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::Interface as _;

struct Registration {
    factory: IDWriteFactory,
    loader: IDWriteInMemoryFontFileLoader,
}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: This factory registered the loader and both references remain live during removal.
        unsafe {
            let _ = self.factory.UnregisterFontFileLoader(&self.loader);
        }
    }
}

pub(super) struct MemoryFile {
    blob: WeakBlob<u8>,
    pub(super) file: IDWriteFontFile,
    _registration: Registration,
}

static FILES: LazyLock<Mutex<HashMap<u64, Arc<MemoryFile>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn file(factory: &IDWriteFactory, blob: &Blob<u8>) -> Option<Arc<MemoryFile>> {
    let mut files = FILES.lock().ok()?;
    if let Some(entry) = files.get(&blob.id()) {
        return Some(Arc::clone(entry));
    }
    files.retain(|_, entry| entry.blob.upgrade().is_some());
    let factory5 = factory.cast::<IDWriteFactory5>().ok()?;
    let size = u32::try_from(blob.len()).ok()?;
    // SAFETY: The live factory creates and registers the loader returned by DirectWrite.
    let loader = unsafe {
        let loader = factory5.CreateInMemoryFontFileLoader().ok()?;
        factory.RegisterFontFileLoader(&loader).ok()?;
        loader
    };
    let registration = Registration {
        factory: factory.clone(),
        loader,
    };
    // SAFETY: The blob covers size bytes for the call and a null owner asks DirectWrite to copy them.
    let file = unsafe {
        registration.loader.CreateInMemoryFontFileReference(
            factory,
            blob.as_ref().as_ptr().cast(),
            size,
            None,
        )
    }
    .ok()?;
    let entry = Arc::new(MemoryFile {
        blob: blob.downgrade(),
        file,
        _registration: registration,
    });
    files.insert(blob.id(), Arc::clone(&entry));
    Some(entry)
}
