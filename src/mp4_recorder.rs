use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::num::NonZeroU32;
use std::path::Path;
use std::time::SystemTime;

use shiguredo_mp4::TrackKind;
use shiguredo_mp4::Uint;
use shiguredo_mp4::boxes::{Avc1Box, AvccBox, SampleEntry, VisualSampleEntryFields};
use shiguredo_mp4::mux::{Mp4FileMuxer, Mp4FileMuxerOptions, Sample};

use crate::h264_depacketizer::AccessUnit;
use crate::sps::SpsInfo;

const VIDEO_TIMESCALE: u32 = 90_000;
const MAX_PARAMETER_SET_LEN: usize = 2048;

pub struct Mp4Recorder {
    file: File,
    muxer: Mp4FileMuxer,
    file_position: u64,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    sps_info: Option<SpsInfo>,
    video_first_sample: bool,
    pending_au: Option<PendingAccessUnit>,
}

struct PendingAccessUnit {
    data: Vec<u8>,
    timestamp: u32,
    keyframe: bool,
}

impl Mp4Recorder {
    pub fn new(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let creation_timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
        let options = Mp4FileMuxerOptions {
            creation_timestamp,
            ..Default::default()
        };
        let muxer = Mp4FileMuxer::with_options(options)?;
        let initial_bytes = muxer.initial_boxes_bytes().to_vec();

        let mut file = File::create(path)?;
        file.write_all(&initial_bytes)?;

        Ok(Self {
            file,
            muxer,
            file_position: initial_bytes.len() as u64,
            sps: None,
            pps: None,
            sps_info: None,
            video_first_sample: true,
            pending_au: None,
        })
    }

    pub fn set_sps_pps(&mut self, sps: Vec<u8>, pps: Vec<u8>) {
        if !is_valid_parameter_set(&sps, 7) || !is_valid_parameter_set(&pps, 8) {
            return;
        }
        self.sps_info = crate::sps::parse_sps(&sps);
        self.sps = Some(sps);
        self.pps = Some(pps);
    }

    pub fn update_sps_pps_if_available(&mut self, sps: Option<&Vec<u8>>, pps: Option<&Vec<u8>>) {
        if let Some(sps) = sps
            && self.sps.as_ref() != Some(sps)
            && is_valid_parameter_set(sps, 7)
        {
            self.sps_info = crate::sps::parse_sps(sps);
            self.sps = Some(sps.clone());
        }
        if let Some(pps) = pps
            && self.pps.as_ref() != Some(pps)
            && is_valid_parameter_set(pps, 8)
        {
            self.pps = Some(pps.clone());
        }
    }

    pub fn write_access_unit(&mut self, au: &AccessUnit) -> Result<(), Box<dyn std::error::Error>> {
        if self.sps.is_none() || self.pps.is_none() || self.sps_info.is_none() {
            return Ok(());
        }

        if let Some(pending) = self.pending_au.take() {
            let duration = au.timestamp.wrapping_sub(pending.timestamp);
            self.write_video_sample(&pending.data, pending.keyframe, duration)?;
        }

        self.pending_au = Some(PendingAccessUnit {
            data: au.data.clone(),
            timestamp: au.timestamp,
            keyframe: au.keyframe,
        });

        Ok(())
    }

    pub fn finalize(mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(pending) = self.pending_au.take() {
            self.write_video_sample(&pending.data, pending.keyframe, 3_000)?;
        }

        let finalized = self.muxer.finalize()?;
        for (offset, bytes) in finalized.offset_and_bytes_pairs() {
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(bytes)?;
        }
        self.file.flush()?;
        Ok(())
    }

    fn write_video_sample(
        &mut self,
        data: &[u8],
        keyframe: bool,
        duration: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.file.write_all(data)?;

        let sample_entry = if self.video_first_sample {
            self.video_first_sample = false;
            Some(self.build_video_sample_entry())
        } else {
            None
        };

        let sample = Sample {
            track_kind: TrackKind::Video,
            sample_entry,
            keyframe,
            timescale: NonZeroU32::new(VIDEO_TIMESCALE).expect("non-zero"),
            duration,
            data_offset: self.file_position,
            data_size: data.len(),
        };
        self.muxer.append_sample(&sample)?;
        self.file_position += data.len() as u64;
        Ok(())
    }

    fn build_video_sample_entry(&self) -> SampleEntry {
        let sps_info = self.sps_info.as_ref().expect("sps info required");
        let sps = self.sps.as_ref().expect("sps required");
        let pps = self.pps.as_ref().expect("pps required");

        let is_high_profile = !matches!(sps_info.profile_idc, 66 | 77 | 88);

        let avcc_box = AvccBox {
            avc_profile_indication: sps_info.profile_idc,
            profile_compatibility: sps_info.profile_compatibility,
            avc_level_indication: sps_info.level_idc,
            length_size_minus_one: Uint::new(3),
            sps_list: vec![sps.clone()],
            pps_list: vec![pps.clone()],
            chroma_format: if is_high_profile {
                Some(Uint::new(sps_info.chroma_format_idc))
            } else {
                None
            },
            bit_depth_luma_minus8: if is_high_profile {
                Some(Uint::new(sps_info.bit_depth_luma_minus8))
            } else {
                None
            },
            bit_depth_chroma_minus8: if is_high_profile {
                Some(Uint::new(sps_info.bit_depth_chroma_minus8))
            } else {
                None
            },
            sps_ext_list: vec![],
        };

        SampleEntry::Avc1(Avc1Box {
            visual: VisualSampleEntryFields {
                data_reference_index: VisualSampleEntryFields::DEFAULT_DATA_REFERENCE_INDEX,
                width: sps_info.width,
                height: sps_info.height,
                horizresolution: VisualSampleEntryFields::DEFAULT_HORIZRESOLUTION,
                vertresolution: VisualSampleEntryFields::DEFAULT_VERTRESOLUTION,
                frame_count: VisualSampleEntryFields::DEFAULT_FRAME_COUNT,
                compressorname: VisualSampleEntryFields::NULL_COMPRESSORNAME,
                depth: VisualSampleEntryFields::DEFAULT_DEPTH,
            },
            avcc_box,
            unknown_boxes: vec![],
        })
    }
}

fn is_valid_parameter_set(nal: &[u8], expected_type: u8) -> bool {
    if nal.is_empty() || nal.len() > MAX_PARAMETER_SET_LEN {
        return false;
    }
    (nal[0] & 0x1F) == expected_type
}
