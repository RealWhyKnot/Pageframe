use std::mem::ManuallyDrop;
use std::sync::Arc;

use num_traits::ToPrimitive;
use url::Url;
use windows::Win32::Foundation::{E_ABORT, E_FAIL};
use windows::Win32::Media::MediaFoundation::{
    IMFMediaSource, IMFSample, IMFSourceReader, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_OBJECT_INVALID,
    MF_PD_DURATION, MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE,
    MF_RESOLUTION_MEDIASOURCE, MF_RESOLUTION_READ, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, MF_SOURCE_READER_MEDIASOURCE,
    MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READERF_ERROR, MFCreateAttributes, MFCreateMFByteStreamOnStream, MFCreateMediaType,
    MFCreateSourceReaderFromMediaSource, MFCreateSourceResolver,
};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0, PropVariantClear,
};
use windows::Win32::System::Variant::{VT_I8, VT_UI8};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows_core::{GUID, HSTRING, Interface};

use super::{Failure, MEDIA_ERR_SRC_NOT_SUPPORTED, Shared, proceed, source, stream, unsupported};
use crate::http::HttpClient;

const TICKS: f64 = 10_000_000.0;

pub(super) fn stream_index(index: i32) -> u32 {
    index.cast_unsigned()
}

fn seconds(ticks: i64) -> f64 {
    ticks.to_f64().unwrap_or(0.0) / TICKS
}

pub(super) fn open_reader(
    url: &Url,
    http: &HttpClient,
    shared: &Arc<Shared>,
) -> Result<IMFSourceReader, Failure> {
    let mut source = source::open(url, http, shared)?;
    let hint = HSTRING::from(format!(
        "media{}",
        source::sniff_extension(source.as_mut(), url)
    ));
    proceed(shared)?;
    let stream = stream::SourceStream::wrap(source, Arc::clone(shared));
    let flags = MF_RESOLUTION_MEDIASOURCE.0
        | MF_RESOLUTION_CONTENT_DOES_NOT_HAVE_TO_MATCH_EXTENSION_OR_MIME_TYPE.0
        | MF_RESOLUTION_READ.0;
    // SAFETY: every pointer passed below is a live local or COM object owned by this thread for the call.
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&raw mut attributes, 1).map_err(|error| unsupported(&error))?;
        let attributes = attributes
            .ok_or_else(|| Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, "no reader attributes"))?;
        attributes
            .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
            .map_err(|error| unsupported(&error))?;
        let bytes = MFCreateMFByteStreamOnStream(&stream).map_err(|error| unsupported(&error))?;
        let resolver = MFCreateSourceResolver().map_err(|error| unsupported(&error))?;
        let mut kind = MF_OBJECT_INVALID;
        let mut object = None;
        resolver
            .CreateObjectFromByteStream(
                &bytes,
                &hint,
                flags.cast_unsigned(),
                None::<&IPropertyStore>,
                &raw mut kind,
                &raw mut object,
            )
            .map_err(|error| unsupported(&error))?;
        let media: IMFMediaSource = object
            .ok_or_else(|| Failure::new(MEDIA_ERR_SRC_NOT_SUPPORTED, "no media source"))?
            .cast()
            .map_err(|error| unsupported(&error))?;
        match MFCreateSourceReaderFromMediaSource(&media, &attributes) {
            Ok(reader) => Ok(reader),
            Err(error) => {
                if let Err(shutdown) = media.Shutdown() {
                    tracing::warn!(%shutdown, "Media source shutdown failed after reader creation failed");
                }
                Err(unsupported(&error))
            }
        }
    }
}

pub(super) fn select(
    reader: &IMFSourceReader,
    stream: i32,
    major: &GUID,
    subtype: &GUID,
) -> windows_core::Result<()> {
    // SAFETY: the reader is a live IMFSourceReader and the GUID references outlive each call.
    unsafe {
        reader.SetStreamSelection(stream_index(MF_SOURCE_READER_ALL_STREAMS.0), false)?;
        reader.SetStreamSelection(stream_index(stream), true)?;
        let media_type = MFCreateMediaType()?;
        media_type.SetGUID(&MF_MT_MAJOR_TYPE, major)?;
        media_type.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        reader.SetCurrentMediaType(stream_index(stream), None, &media_type)
    }
}

pub(super) fn has_stream(reader: &IMFSourceReader, stream: i32) -> bool {
    // SAFETY: the reader is a live IMFSourceReader owned by this thread.
    unsafe { reader.GetNativeMediaType(stream_index(stream), 0).is_ok() }
}

pub(super) fn duration(reader: &IMFSourceReader) -> f64 {
    // SAFETY: the reader is live; the PROPVARIANT is read by its tag and cleared after use.
    unsafe {
        let Ok(mut value) = reader.GetPresentationAttribute(
            stream_index(MF_SOURCE_READER_MEDIASOURCE.0),
            &MF_PD_DURATION,
        ) else {
            return f64::INFINITY;
        };
        let inner = &value.Anonymous.Anonymous;
        let result = if inner.vt == VT_UI8 {
            inner.Anonymous.uhVal.to_f64().unwrap_or(0.0) / TICKS
        } else {
            f64::INFINITY
        };
        let _ = PropVariantClear(&raw mut value);
        if result > 0.0 { result } else { f64::INFINITY }
    }
}

pub(super) fn set_position(reader: &IMFSourceReader, seconds: f64) -> windows_core::Result<()> {
    let ticks = (seconds * TICKS)
        .round()
        .to_i64()
        .ok_or_else(|| windows_core::Error::from(windows::Win32::Foundation::E_INVALIDARG))?;
    let value = PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_I8,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 { hVal: ticks },
            }),
        },
    };
    // SAFETY: `value` is a VT_I8 PROPVARIANT that lives across the call and owns no resources.
    unsafe { reader.SetCurrentPosition(&GUID::zeroed(), &raw const value) }
}

pub(super) struct Sample {
    pub(super) data: Option<(IMFSample, f64)>,
    pub(super) changed: bool,
    pub(super) end: bool,
}

pub(super) struct Reader(Option<IMFSourceReader>);

impl Reader {
    pub(super) fn new(reader: IMFSourceReader) -> Self {
        Self(Some(reader))
    }

    pub(super) fn is_retired(&self) -> bool {
        self.0.is_none()
    }

    pub(super) fn get(&self) -> windows_core::Result<&IMFSourceReader> {
        self.0.as_ref().ok_or_else(|| E_FAIL.into())
    }

    pub(super) fn read_sample(
        &mut self,
        stream: i32,
        shared: &Shared,
    ) -> windows_core::Result<Sample> {
        if shared.is_stopped() {
            return Err(E_ABORT.into());
        }
        let reader = self.get()?;
        let mut flags = 0_u32;
        let mut time = 0_i64;
        let mut sample = None;
        // SAFETY: the out pointers reference locals that live across the synchronous ReadSample call.
        let result = unsafe {
            reader.ReadSample(
                stream_index(stream),
                0,
                None,
                Some(&raw mut flags),
                Some(&raw mut time),
                Some(&raw mut sample),
            )
        };
        let flags = flags.cast_signed();
        if flags & MF_SOURCE_READERF_ERROR.0 != 0 {
            self.0 = None;
            return result.and(Err(E_FAIL.into()));
        }
        result?;
        Ok(Sample {
            data: sample.map(|sample| (sample, seconds(time))),
            changed: flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 != 0,
            end: flags & MF_SOURCE_READERF_ENDOFSTREAM.0 != 0,
        })
    }
}
