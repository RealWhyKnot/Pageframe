use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    E_INVALIDARG, E_NOTIMPL, E_POINTER, S_FALSE, S_OK, STG_E_ACCESSDENIED, STG_E_INVALIDFUNCTION,
    STG_E_READFAULT,
};
use windows::Win32::System::Com::{
    ISequentialStream_Impl, IStream, IStream_Impl, LOCKTYPE, STATFLAG, STATSTG, STGC, STGTY_STREAM,
    STREAM_SEEK, STREAM_SEEK_CUR, STREAM_SEEK_END, STREAM_SEEK_SET,
};
use windows_core::{HRESULT, Ref, implement};

use crate::media::shared::Shared;
use crate::media::source::{ByteSource, network_failure};

#[implement(IStream)]
pub(super) struct SourceStream {
    source: Mutex<Box<dyn ByteSource>>,
    shared: Arc<Shared>,
}

impl SourceStream {
    pub(super) fn wrap(source: Box<dyn ByteSource>, shared: Arc<Shared>) -> IStream {
        Self {
            source: Mutex::new(source),
            shared,
        }
        .into()
    }

    fn with<T>(&self, run: impl FnOnce(&mut dyn ByteSource) -> T) -> T {
        let mut source = self.source.lock().unwrap_or_else(PoisonError::into_inner);
        run(source.as_mut())
    }
}

fn report_read(pcbread: *mut u32, count: u32) {
    if !pcbread.is_null() {
        // SAFETY: `pcbread` is a non-null out pointer supplied by the COM caller for this call.
        unsafe { *pcbread = count };
    }
}

impl ISequentialStream_Impl for SourceStream_Impl {
    fn Read(&self, pv: *mut core::ffi::c_void, cb: u32, pcbread: *mut u32) -> HRESULT {
        report_read(pcbread, 0);
        if cb == 0 {
            return S_OK;
        }
        if pv.is_null() {
            return E_POINTER;
        }
        let Ok(len) = usize::try_from(cb) else {
            return E_INVALIDARG;
        };
        if len > isize::MAX.unsigned_abs() {
            return E_INVALIDARG;
        }
        // SAFETY: COM callers pass a writable buffer of at least `cb` bytes at `pv`, checked non-null above.
        let buffer = unsafe { std::slice::from_raw_parts_mut(pv.cast::<u8>(), len) };
        let deadline = Instant::now() + Duration::from_secs(10);
        let read = self.with(|source| {
            let mut total = 0;
            while total < buffer.len() {
                if self.shared.is_stopped() {
                    return Err((total, std::io::ErrorKind::ConnectionAborted.into()));
                }
                match source.read_until(&mut buffer[total..], deadline) {
                    Ok(0) => break,
                    Ok(read) => total += read,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err((total, error)),
                }
            }
            Ok(total)
        });
        match read {
            Ok(total) => {
                report_read(pcbread, u32::try_from(total).unwrap_or(cb));
                if total < len { S_FALSE } else { S_OK }
            }
            Err((total, error)) => {
                self.shared.record_network(network_failure(&error));
                report_read(pcbread, u32::try_from(total).unwrap_or(cb));
                STG_E_READFAULT
            }
        }
    }

    fn Write(&self, _: *const core::ffi::c_void, _: u32, _: *mut u32) -> HRESULT {
        STG_E_ACCESSDENIED
    }
}

impl IStream_Impl for SourceStream_Impl {
    fn Seek(
        &self,
        dlibmove: i64,
        dworigin: STREAM_SEEK,
        plibnewposition: *mut u64,
    ) -> windows_core::Result<()> {
        let position = self.with(|source| {
            let base = match dworigin {
                STREAM_SEEK_SET => Some(0),
                STREAM_SEEK_CUR => Some(source.position()),
                STREAM_SEEK_END => Some(source.len()),
                _ => None,
            }?;
            let target = base.checked_add_signed(dlibmove)?;
            source.set_position(target);
            Some(source.position())
        });
        let Some(position) = position else {
            return Err(STG_E_INVALIDFUNCTION.into());
        };
        if !plibnewposition.is_null() {
            // SAFETY: `plibnewposition` is a non-null out pointer supplied by the COM caller for this call.
            unsafe { *plibnewposition = position };
        }
        Ok(())
    }

    fn SetSize(&self, _: u64) -> windows_core::Result<()> {
        Err(STG_E_ACCESSDENIED.into())
    }

    fn CopyTo(
        &self,
        _: Ref<IStream>,
        _: u64,
        _: *mut u64,
        _: *mut u64,
    ) -> windows_core::Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Commit(&self, _: &STGC) -> windows_core::Result<()> {
        Ok(())
    }

    fn Revert(&self) -> windows_core::Result<()> {
        Ok(())
    }

    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> windows_core::Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }

    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> windows_core::Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }

    fn Stat(&self, pstatstg: *mut STATSTG, _: &STATFLAG) -> windows_core::Result<()> {
        if pstatstg.is_null() {
            return Err(E_POINTER.into());
        }
        let size = self.with(|source| source.len());
        let stat = STATSTG {
            r#type: u32::try_from(STGTY_STREAM.0).unwrap_or(2),
            cbSize: size,
            ..STATSTG::default()
        };
        // SAFETY: `pstatstg` is a non-null, writable STATSTG supplied by the COM caller for this call.
        unsafe { pstatstg.write(stat) };
        Ok(())
    }

    fn Clone(&self) -> windows_core::Result<IStream> {
        Err(E_NOTIMPL.into())
    }
}
