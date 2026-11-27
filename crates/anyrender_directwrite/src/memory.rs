use peniko::{Blob, WeakBlob};
use std::collections::HashMap;
use std::rc::Rc;
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

pub(super) struct MemoryFiles {
    files: HashMap<u64, Rc<MemoryFile>>,
    factory: IDWriteFactory,
}

impl MemoryFiles {
    pub(super) fn new(factory: IDWriteFactory) -> Self {
        Self {
            files: HashMap::new(),
            factory,
        }
    }

    pub(super) fn factory(&self) -> &IDWriteFactory {
        &self.factory
    }

    pub(super) fn file(&mut self, blob: &Blob<u8>) -> Option<Rc<MemoryFile>> {
        if let Some(file) = self.files.get(&blob.id()) {
            return Some(Rc::clone(file));
        }
        self.files.retain(|_, file| file.blob.upgrade().is_some());
        let file = Rc::new(create_file(&self.factory, blob)?);
        self.files.insert(blob.id(), Rc::clone(&file));
        Some(file)
    }
}

fn create_file(factory: &IDWriteFactory, blob: &Blob<u8>) -> Option<MemoryFile> {
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
    Some(MemoryFile {
        blob: blob.downgrade(),
        file,
        _registration: registration,
    })
}
