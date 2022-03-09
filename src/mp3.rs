use std::{error::Error, fmt};
use std::io::Read;

use crate::mp3::Emphasis::None;
use crate::mp3::ProtectionBit::Protected;

// These constants are for parsing the various portions of the MP3 Frame header. The
// bits set to True in these constants are the bits used by that section of the header.
// See the link below for further details.
// https://www.datavoyage.com/mpgscript/mpeghdr.htm
const SYNC_WORD: u32 =          0xFF_E0_00_00; // 11111111 11100000 00000000 00000000
const MPEG_VERSION_ID: u32 =    0x00_18_00_00; // 00000000 00011000 00000000 00000000
const LAYER_DESCRIPTION: u32 =  0x00_06_00_00; // 00000000 00000110 00000000 00000000
const PROTECTION_BIT: u32 =     0x00_01_00_00; // 00000000 00000001 00000000 00000000
const BITRATE_INDEX: u32 =      0x00_00_F0_00; // 00000000 00000000 11110000 00000000
const SAMPLE_FREQ: u32 =        0x00_00_0C_00; // 00000000 00000000 00001100 00000000
const PADDING_BIT: u32 =        0x00_00_02_00; // 00000000 00000000 00000010 00000000
const PRIVATE_BIT: u32 =        0x00_00_01_00; // 00000000 00000000 00000001 00000000
const CHANNEL_MODE: u32 =       0x00_00_00_C0; // 00000000 00000000 00000000 11000000
const MODE_EXT: u32 =           0x00_00_00_30; // 00000000 00000000 00000000 00110000
const COPYRIGHT: u32 =          0x00_00_00_08; // 00000000 00000000 00000000 00001000
const ORIGINAL: u32 =           0x00_00_00_04; // 00000000 00000000 00000000 00000100
const EMPHASIS: u32 =           0x00_00_00_03; // 00000000 00000000 00000000 00000011

/// MPEG Audio version ID
// TODO: manually implement these traits to reduce compile times.
#[derive(Clone, Copy, PartialEq, Debug, Clone, Copy)]
enum MpegVersion
{
    Version25,  // MPEG Version 2.5 (00)
    // Reserved bit combination (01)
    Version2,   // MPEG Version 2 (10)
    Version1,   // MPEG Version 1 (11)
}

// Layer Description
#[derive(Clone, Copy, PartialEq, Debug)]
enum LayerDesc
{
    // Reserved bit combination (00)
    Layer3,     // Layer III (01)
    Layer2,     // Layer II (10)
    Layer1,     // Layer I (11)
}

// Protection bit
#[derive(Debug, PartialEq, Copy, Clone)]
enum ProtectionBit
{
    Protected, // Protected by following 16 bit CRC header (0)
    Unprotected, // Not protected (1)
}
// Channel Mode
#[derive(PartialEq, Debug, Copy, Clone)]
enum ChannelMode
{
    Stereo,
    JointStereo,    // Stereo
    DualChannel,    // 2 Mono Channels
    SingleChannel,  // Mono
}
#[derive(Copy, Clone, PartialEq, Debug)]
enum Emphasis
{
    None,
    Ms5015,
    CcitJ17,
}

// Audio Layer I/II/II frame header
#[derive(Copy, Clone)]
struct FrameHeader
{
    mpeg_version: MpegVersion,      // MPEG Version of the frame
    layer_desc: LayerDesc,          // MPEG layer of the frame
    protection_bit: ProtectionBit,  // If true, no 16 bit CRC follows the header
    bit_rate: u32,                  // The bitrate for the frame
    sample_rate: u32,               // The sample rate of the frame in bits per second
    padded: bool,                   // If true, use a padding slot to fit the bitrate
    private: bool,                  // Informative only
    channel_mode: ChannelMode,      // Channel model of the frame
    mode_ext_band: Option<u8>,      // Only used in Layer I & II joint stereo. The value is the start band.
    intensity_stereo: Option<bool>, // Only used in Layer III joint stereo.
    ms_stereo: Option<bool>,        // Only used in Layer III joint stereo.
    copy_righted: bool,             // Has the same meaning as the copyright bit on CDs
    original: bool,                 // If true, the frame presides on its original media
    emphasis: Emphasis,             // Tells the de-coder to de-emphasize the file during decoding, is rarely used
}

// TODO: Make errors more granular by specifying what is wrong in the header, rather than just specifying
//  that the header is invalid
// Error Invalid Headers
#[derive(Debug, PartialEq)]
struct FrameHeaderError
{
    details: String
}

impl FrameHeaderError
{
    fn new(msg: &str) -> FrameHeaderError
    {
        FrameHeaderError{details: msg.to_string()}
    }
}

impl fmt::Display for FrameHeaderError
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "{}", self.details)
    }
}

impl Error for FrameHeaderError
{
    fn description(&self) -> &str
    {
        return &self.details;
    }
}

impl FrameHeader
{
    // Returns the Bitrate for a given combination of Mpeg Version, Layer Desc, and bits
    // using a lookup table.
    fn decode_bitrate(bits: u32, ver: MpegVersion, layer: LayerDesc) -> u32
    {
        // Bitrates in bits per second
        static BITRATE_VALUES: [[u32; 5]; 15] = [
            [0,         0,          0,          0,          0],
            [32_000,    32_000,     32_000,     32_000,     8_000],
            [64_000,    48_000,     40_000,     48_000,     16_000],
            [96_000,    56_000,     48_000,     56_000,     24_000],
            [128_000,   64_000,     56_000,     64_000,     32_000],
            [160_000,   80_000,     64_000,     80_000,     40_000],
            [192_000,   96_000,     80_000,     96_000,     48_000],
            [224_000,   112_000,    96_000,     112_000,    56_000],
            [256_000,   128_000,    112_000,    128_000,    64_000],
            [288_000,   160_000,    128_000,    144_000,    80_000],
            [320_000,   192_000,    160_000,    160_000,    96_000],
            [352_000,   224_000,    192_000,    176_000,    112_000],
            [384_000,   256_000,    224_000,    192_000,    128_000],
            [416_000,   320_000,    256_000,    224_000,    144_000],
            [448_000,   384_000,    320_000,    256_000,    160_000],
        ];
        if bits == 0
        {
            return 0
        }

        // TODO: For some reason, if this isn't made mutable, the unit tests fail.
        let mut look_up = 0;
        match ver
        {
            MpegVersion::Version1 => {
                look_up = match layer
                {
                    LayerDesc::Layer1 => 0,
                    LayerDesc::Layer2 => 1,
                    LayerDesc::Layer3 => 2,
                };
            }
            MpegVersion::Version2 => {
                look_up = match layer
                {
                    LayerDesc::Layer1 => 3,
                    LayerDesc::Layer2 => 4,
                    LayerDesc::Layer3 => 4,
                };
            },
            MpegVersion::Version25 => {
                look_up = match layer
                {
                    LayerDesc::Layer1 => 3,
                    LayerDesc::Layer2 => 4,
                    LayerDesc::Layer3 => 4,
                };
            }
        }
        return BITRATE_VALUES[bits as usize][look_up as usize];
    }
    // Returns the sample rate for a given MPEG Version and sampling rate index using a lookup table
    fn decode_sample_rate(bits: u32, ver: MpegVersion) -> u32
    {
        // Sampling Rate Frequencies in Hz
        static SAMPLING_RATES: [[u32; 3]; 3] = [
            [44_100,    22_050,     11_025,],
            [48_000,    24_000,     12_000,],
            [32_000,    16_000,     8_000,],
        ];
        let look_up: u32 = match ver {
            MpegVersion::Version1 => 0,
            MpegVersion::Version2 => 1,
            MpegVersion::Version25 => 2,
        };
        return SAMPLING_RATES[bits as usize][look_up as usize];
    }

    // Accepts a slice of four u8 values and returns either FrameHeader or a FrameHeaderError
    // for invalid headers.
    fn new(slice: [u8; 4]) -> Result<FrameHeader, FrameHeaderError>
    {
        let value = u32::from_be_bytes(slice);

        // Check for the sync word in the first 12 bits. Something bit-wise AND'd with itself
        // is itself. If the sync-word is missing a different value will be produced.
        if SYNC_WORD & value != SYNC_WORD
        {
            return Err(FrameHeaderError::new("Sync word not found!"));
        }

        // Check the MPEG Version ID. The value compared against is (True, False) for bits 20 and
        // 19 of the frame header. This is a reserved combination.
        let mpeg_version = match (MPEG_VERSION_ID & value) >> 19
        {
            0b00 => MpegVersion::Version25,
            0b01 => return Err(FrameHeaderError::new("Reserved value '0b01' used for MPEG Version ID!")),
            0b10 => MpegVersion::Version2,
            0b11 => MpegVersion::Version1,
            _    => return Err(FrameHeaderError::new("Error encountered when parsing MPEG Version ID!")),
        };
        // Check the Layer Description of the header. The combination of the bits, 18 and 17, used
        // for this section cannot both be False. That is a reserved combination.
        let layer_desc = match (LAYER_DESCRIPTION & value) >> 17
        {
            0b00 => return Err(FrameHeaderError::new("Reserved value '0b00' used for Layer Description!")),
            0b01 => LayerDesc::Layer3,
            0b10 => LayerDesc::Layer2,
            0b11 => LayerDesc::Layer1,
            _    => return Err(FrameHeaderError::new("Error encountered when parsing Layer Description!")),
        };
        let unprotected = match (PROTECTION_BIT & value) >> 16
        {
            0b0 => ProtectionBit::Protected,
            0b1 => ProtectionBit::Unprotected,
            _   => return Err(FrameHeaderError::new("Error encountered when parsing protection bit!")),
        };
        // Lookup the bit rate using bits 15 through 12. The value 0b1111 is an invalid value.
        let bit_rate = match (BITRATE_INDEX & value) >> 12
        {
            0b1111 => return Err(FrameHeaderError::new("Invalid value '0b1111' for Bitrate index!")),
            _ => FrameHeader::decode_bitrate((BITRATE_INDEX & value) >> 12, mpeg_version, layer_desc)
        };
        // Lookup the sampling rate frequency using bits 11 through 10, The value 0b11 is a reserved value.
        let sample_rate = match (SAMPLE_FREQ & value) >> 10
        {
            0b11 => return Err(FrameHeaderError::new("Reserved value '0b11' used for sampling rate index!")),
            _ => FrameHeader::decode_sample_rate((SAMPLE_FREQ & value) >> 10, mpeg_version),
        };
        let padded =  ((PADDING_BIT & value) >> 9) != 0;
        let private = ((PRIVATE_BIT & value) >> 8) != 0;
        let channel_mode = match (CHANNEL_MODE & value) >> 6
        {
            0b00 => ChannelMode::Stereo,
            0b01 => ChannelMode::JointStereo,
            0b10 => ChannelMode::DualChannel,
            0b11 => ChannelMode::SingleChannel,
            _ => return Err(FrameHeaderError::new("Error encountered when parsing channel mode!"))
        };
        let mut mode_ext_band: Option<u8> = None;
        let mut intensity_stereo: Option<bool> = None;
        let mut ms_stereo: Option<bool> = None;

        if channel_mode == ChannelMode::JointStereo
        {
            if layer_desc == LayerDesc::Layer1 || layer_desc == LayerDesc::Layer2
            {
                mode_ext_band = match (MODE_EXT & value) >> 4
                {
                    0b00 => Some(4),
                    0b01 => Some(8),
                    0b10 => Some(12),
                    0b11 => Some(16),
                    _    => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
                let intensity_stereo: Option<bool> = None;
                let  ms_stereo: Option<bool> = None;
            }
            else
            {
                let mode_ext_band: Option<u8> = None;
                intensity_stereo = match (MODE_EXT & value) >> 4
                {
                    0b00 => Some(false),
                    0b01 => Some(true),
                    0b10 => Some(false),
                    0b11 => Some(true),
                    _    => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
                ms_stereo = match (MODE_EXT & value) >> 4
                {
                    0b00 => Some(false),
                    0b01 => Some(false),
                    0b10 => Some(true),
                    0b11 => Some(true),
                    _   => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
            }
        }
        let copy_righted =  ((COPYRIGHT & value) >> 3) != 0;
        let original = ((ORIGINAL & value) >> 2) != 0;
        let emphasis = match EMPHASIS & value
        {
            0b00 => Emphasis::None,
            0b01 => Emphasis::Ms5015,
            0b10 => return Err(FrameHeaderError::new("Reserved value '0b10' used for emphasis!")),
            0b11 => Emphasis::CcitJ17,
            _ => return Err(FrameHeaderError::new("Error encountered when parsing emphasis!"))
        };


        // For Layer II MP3s, some combinations of bitrate and channel mode are invalid and should return an error
        if layer_desc == LayerDesc::Layer2
        {
            if channel_mode != ChannelMode::SingleChannel
            {
                match bit_rate
                {
                    32_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    48_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    56_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    80_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    _      => (),
                }
            }
            else
            {
                match bit_rate
                {
                    224_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    256_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    320_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    384_000 => return Err(FrameHeaderError::new("Prohibited bitrate and chanel mode for Layer II encountered!")),
                    _       => (),
                }
            }
        }
        return Ok(
            FrameHeader {
                mpeg_version,
                layer_desc,
                protection_bit: unprotected,
                bit_rate,
                sample_rate,
                padded,
                private,
                channel_mode,
                mode_ext_band,
                intensity_stereo,
                ms_stereo,
                copy_righted,
                original,
                emphasis,
            }
        )
    }
    /// Calculates the frame length in bytes based on the frame header values. Note, the frame length is the
    /// length of a frame when compressed. See section G of https://www.codeproject.com/Articles/8295/MPEG-Audio-Frame-Header
    fn calc_frame_len(&self) -> u32
    {
        static SAMPLES_PER_FRAME: [[u32; 3]; 3] = [
            [384,   384,    384],
            [1152,  1152,   1152],
            [1152,  576,    576],
        ];

        let mut row = 0;
        let mut col = 0;

        if self.layer_desc == LayerDesc::Layer2
        {
            row = row + 1;
        }
        else if self.layer_desc == LayerDesc::Layer3
        {
            row = row + 2;
        }
        if self.mpeg_version == MpegVersion::Version2
        {
            col = col + 1;
        }
        else if self.mpeg_version == MpegVersion::Version25
        {
            col = col + 2;
        }
        let samples = SAMPLES_PER_FRAME[row][col];
        let padding: u32 = match self.padded
        {
            true => 1,
            false => 0,
        };
        // TODO: Replace this with the more accurate frame length calculation described in the official MP3 standard
        if self.protection_bit == ProtectionBit::Protected
        {
            // If the protection bit isn't set, then a 16 bit (2 Byte) CRC proceeds after the header
            // and before the data.
            return (samples * self.bit_rate) / (8 * self.sample_rate)  + padding + 2;
        }
        return (samples * self.bit_rate) / (8 * self.sample_rate)  + padding;
    }
}

// Represents an MP3 frame. Each frame contains a header struct and a vector of the bytes
// of the data portion of the frame.
struct Frame
{
    header: Result<FrameHeader, FrameHeaderError>,
    data: Vec<u8>,
}

// Represents a parsed MP3 file as a sequence of repeating parsed MP3 frames
struct Mp3
{
    frames: Vec<Frame>,
    len: u32,
}

impl Mp3
{
    // Parses an input with the `Read` trait and returns a Mp3.
    fn new(mut data: impl Read) -> Mp3
    {
        let parsed_mp3 = Mp3 { frames: Vec::new(), len: 0 };

        // Read the data in one kilobyte at a time
        let mut buffer = [0; 1024];

        // https://stackoverflow.com/questions/26379097/reading-bytes-from-a-reader
        while let Ok(bytes_read) = &data.read(&mut buffer)
        {

        }
        return parsed_mp3;
    }
}

// TODO: Consolidate and organize these tests
#[cfg(test)]
mod tests
{
    use super::*;
    use crate::mp3::LayerDesc::Layer1;

    // This test case verifies the FrameHeader::decode_bitrate() method.
    #[test]
    fn test_decode_bitrate()
    {
        // All combinations with the '0b0000' Bit Index should return 0 (Free)
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(0, FrameHeader::decode_bitrate(0b0000, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0001
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(8_000, FrameHeader::decode_bitrate(0b00001, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(8_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(8_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(8_000, FrameHeader::decode_bitrate(0b0001, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0010
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(40_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(16_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(16_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(16_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(16_000, FrameHeader::decode_bitrate(0b0010, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0011
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(24_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(24_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(24_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(24_000, FrameHeader::decode_bitrate(0b0011, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0100
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(32_000, FrameHeader::decode_bitrate(0b0100, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0101
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(40_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(40_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(40_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(40_000, FrameHeader::decode_bitrate(0b0101, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0110
        assert_eq!(192_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(48_000, FrameHeader::decode_bitrate(0b0110, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b0111
        assert_eq!(224_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(56_000, FrameHeader::decode_bitrate(0b0111, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b1000
        assert_eq!(256_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(64_000, FrameHeader::decode_bitrate(0b1000, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value 0b1001
        assert_eq!(288_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(80_000, FrameHeader::decode_bitrate(0b1001, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value of 0b1010
        assert_eq!(320_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(192_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(96_000, FrameHeader::decode_bitrate(0b1010, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value 0b1011
        assert_eq!(352_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(224_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(192_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(176_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(176_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(112_000, FrameHeader::decode_bitrate(0b1011, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value 0b1100
        assert_eq!(384_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(256_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(224_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(192_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(192_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(128_000, FrameHeader::decode_bitrate(0b1100, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value 0b1101
        assert_eq!(416_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(320_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(256_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(224_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(224_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(144_000, FrameHeader::decode_bitrate(0b1101, MpegVersion::Version25, LayerDesc::Layer3));

        // Bitrate Index value 0b1110
        assert_eq!(448_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version1, LayerDesc::Layer1));
        assert_eq!(384_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version1, LayerDesc::Layer2));
        assert_eq!(320_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version1, LayerDesc::Layer3));
        assert_eq!(256_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version2, LayerDesc::Layer1));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version2, LayerDesc::Layer2));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version2, LayerDesc::Layer3));
        assert_eq!(256_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version25, LayerDesc::Layer1));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version25, LayerDesc::Layer2));
        assert_eq!(160_000, FrameHeader::decode_bitrate(0b1110, MpegVersion::Version25, LayerDesc::Layer3));
    }

    // This test case verifies the FrameHeader::decode_sample_rate()
    #[test]
    fn test_decode_sample_rate()
    {
        // Sampling rate index value '0b00'
        assert_eq!(44_100, FrameHeader::decode_sample_rate(0b00,MpegVersion::Version1));
        assert_eq!(22_050, FrameHeader::decode_sample_rate(0b00,MpegVersion::Version2));
        assert_eq!(11_025, FrameHeader::decode_sample_rate(0b00,MpegVersion::Version25));

        // Sampling rate index value '0b01'
        assert_eq!(48_000, FrameHeader::decode_sample_rate(0b01, MpegVersion::Version1));
        assert_eq!(24_000, FrameHeader::decode_sample_rate(0b01, MpegVersion::Version2));
        assert_eq!(12_000, FrameHeader::decode_sample_rate(0b01, MpegVersion::Version25));

        // Sampling rate index '0b10'
        assert_eq!(32_000, FrameHeader::decode_sample_rate(0b10, MpegVersion::Version1));
        assert_eq!(16_000, FrameHeader::decode_sample_rate(0b10, MpegVersion::Version2));
        assert_eq!(8_000, FrameHeader::decode_sample_rate(0b10, MpegVersion::Version25));
    }

    /// Verifies that FrameHeader::new() returns an error if the sync word is missing from the data being parsed
    #[test]
    fn test_frame_header_no_sync_word()
    {
        let data = [0b1011_1011, 0b1111_1000, 0b0000_0000, 0b0000_0000];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Sync word not found!");
    }

    /// Verifies that FrameHeader::new() returns an error if value `0b01` is used for the MPEG Version.
    #[test]
    fn test_frame_header_new_bad_mpeg()
    {
        let data = [0b1111_1111, 0b1110_1000, 0b0000_0000, 0b0000_0000];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Reserved value '0b01' used for MPEG Version ID!");
    }

    /// Verifies that FrameHeader::new() returns an error if `0b00` is used for the Layer Description value
    #[test]
    fn test_frame_header_new_bad_layer_desc()
    {
        let data: [u8; 4] = [0b1111_1111, 0b1111_0000, 0b0000_0000, 0b0000_0000];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Reserved value '0b00' used for Layer Description!");
    }

    /// Verifies that FrameHeader::new() returns an error if `0b1111` is used for the bitrate index
    #[test]
    fn test_frame_header_new_bad_bitrate()
    {
        let data: [u8; 4] = [0b1111_1111, 0b1111_0100, 0b1111_0000, 0b0000_0000];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Invalid value '0b1111' for Bitrate index!");
    }
    /// Verifies that FrameHeader::new() returns an error if `0b11` is used for the sample rate
    #[test]
    fn test_frame_header_new_bad_sample_rate()
    {
        let data: [u8; 4] = [0b1111_1111, 0b1111_0100, 0b1011_1100, 0b0000_0000];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Reserved value '0b11' used for sampling rate index!");
    }
    /// Verifies that FrameHeader::new() returns an error if `0b10` is used for the emphasis value.
    #[test]
    fn test_frame_header_new_bad_emphasis()
    {
        let data: [u8; 4] = [0b1111_1111, 0b1111_0100, 0b1011_1000, 0b0000_0010];
        let x = FrameHeader::new(data);
        assert_eq!(x.err().unwrap().to_string(), "Reserved value '0b10' used for emphasis!");
    }

    /// Verifies that FrameHeader::new() correctly parses the MPEG audio version ID.
    #[test]
    fn test_frame_header_new_mpeg_version()
    {
        // MPEG version 2.5
        let data: [u8; 4] = [0b1111_1111, 0b1110_0100, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().mpeg_version, MpegVersion::Version25);

        // MPEG version 2
        let data: [u8; 4] = [0b1111_1111, 0b1111_0100, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().mpeg_version, MpegVersion::Version2);

        // MPEG version 1
        let data: [u8; 4] = [0b1111_1111, 0b1111_1100, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().mpeg_version, MpegVersion::Version1);
    }
    /// Verifies that FrameHeader::new() correctly parses the layer description.
    #[test]
    fn test_frame_header_new_layer_desc()
    {
        // Layer Description III
        let data: [u8; 4] = [0b1111_1111, 0b1110_0010, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().layer_desc, LayerDesc::Layer3);

        // Layer Description II
        let data: [u8; 4] = [0b1111_1111, 0b1110_0100, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().layer_desc, LayerDesc::Layer2);

        // Layer Description I
        let data: [u8; 4] = [0b1111_1111, 0b1110_0110, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().layer_desc, LayerDesc::Layer1);
    }
    /// Verifies that FrameHeader::new() correctly parses the protection bit.
    #[test]
    fn test_frame_header_new_protect_bit()
    {
        // Protected
        let data: [u8; 4] = [0b1111_1111, 0b1110_0110, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().protection_bit, ProtectionBit::Protected);

        // Protected
        let data: [u8; 4] = [0b1111_1111, 0b1110_0111, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().protection_bit, ProtectionBit::Unprotected);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 1 and Layer 1
    #[test]
    fn test_frame_header_new_bitrate_v1l1()
    {
        // Free bitrate, aka 0.
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0000_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0001_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0010_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0100_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0101_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);

        // 192Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0110_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 192_000);

        // 224Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b0111_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 224_000);

        // 256Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1000_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 256_000);

        // 288Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1001_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 288_000);

        // 320Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1010_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 320_000);

        // 352Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1011_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 352_000);

        // 384Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1100_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 384_000);

        // 416Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1101_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 416_000);

        // 448Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1110_1000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 448_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 1 and Layer II
    #[test]
    fn test_frame_header_new_bitrate_v1l2()
    {
        // Free bitrate, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);

        // 192Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 192_000);

        // 224Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1011_1000, 0b1000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 224_000);

        // 256Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1100_1000, 0b1000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 256_000);

        // 320Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1101_1000, 0b1000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 320_000);

        // 384Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1110_1000, 0b1000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 384_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 1 and Layer III
    #[test]
    fn test_frame_header_new_bitrate_v1l3()
    {
        // Free bitrate, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 40Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 40_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);

        // 192Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 192_000);

        // 224Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 224_000);

        // 256Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 256_000);

        // 320Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 320_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 2 and Layer I
    #[test]
    fn test_frame_header_new_bitrate_v2l1()
    {
        // Free, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 144Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 144_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);

        // 176Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 176_000);

        // 192Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 192_000);

        // 224Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 224_000);

        // 256Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0111, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 256_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 2 and Layer II
    #[test]
    fn test_frame_header_new_bitrate_v2l2()
    {
        // Free, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 8Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 8_000);

        // 16Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 16_000);

        // 24Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 24_000);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 40Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 40_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 144Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 144_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0101, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 2 and Layer III
    #[test]
    fn test_frame_header_new_bitrate_v2l3()
    {
        // Free, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 8Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 8_000);

        // 16Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 16_000);

        // 24Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 24_000);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 40Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 40_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 144Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 144_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 25 and Layer II
    #[test]
    fn test_frame_header_new_bitrate_v25l2()
    {
        // Free, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 8Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 8_000);

        // 16Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 16_000);

        // 24Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 24_000);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 40Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 40_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 144Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 144_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0101, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);
    }
    /// Verifies that FrameHeader::new() correctly parses the bitrate index for MPEG Version 2.5 and Layer III
    #[test]
    fn test_frame_header_new_bitrate_v25l3()
    {
        // Free, aka 0
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 0);

        // 8Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 8_000);

        // 16Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 16_000);

        // 24Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 24_000);

        // 32Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 32_000);

        // 40Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 40_000);

        // 48Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 48_000);

        // 56Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b0111_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 56_000);

        // 64Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1000_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 64_000);

        // 80Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1001_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 80_000);

        // 96Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1010_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 96_000);

        // 112Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1011_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 112_000);

        // 128Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1100_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 128_000);

        // 144Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1101_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 144_000);

        // 160Kbps
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().bit_rate, 160_000);
    }

    /// Verifies that FrameHeader::new() correctly parses the sampling rate frequency for MPEG Version 1.
    #[test]
    fn test_frame_header_new_sample_rate_v1()
    {

        // 44.1KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 44_100);

        // 48KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0100, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 48_000);

        // 32KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 32_000);
    }

    /// Verifies that FrameHeader::new() correctly parses the sampling rate frequency for MPEG Version 2
   #[test]
    fn test_frame_header_new_sample_rate_v2()
    {
        // 22.05KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 22_050);

        // 24KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1110_0100, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 24_000);

        // 16KHz
        let data: [u8; 4] = [0b1111_1111, 0b1111_0011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 16_000);
    }

    /// Verifies that FrameHeader::new() correctly parses the sampling rate frequency for MPEG Version 2.5
    #[test]
    fn test_frame_header_new_sample_rate_v25()
    {
        // 11.025KHz
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 11_025);

        // 12KHz
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0100, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 12_000);

        // 8KHz
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_1000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().sample_rate, 8_000);
    }

    /// Verifies that FrameHeader::new() correctly parses the padding bit.
    #[test]
    fn test_frame_header_new_padding()
    {
        // Padding
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().padded, false);

        // No padding
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0010, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().padded, true);
    }

    /// Verifies that FrameHeader::new() correctly parses the private bit
    #[test]
    fn test_frame_header_new_private_bit()
    {
        // Not Private
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().private, false);

        // Private
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0001, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().private, true);
    }
    /// Verifies that FrameHeader::new() correctly parses the channel mode
    #[test]
    fn test_frame_header_new_channel_model()
    {
        // Stereo
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b0000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().channel_mode, ChannelMode::Stereo);

        // Joint Stereo (Stereo)
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().channel_mode, ChannelMode::JointStereo);

        // Dual channel (2 mono channels)
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b1000_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().channel_mode, ChannelMode::DualChannel);

        // Single Channel (Mono)
        let data: [u8; 4] = [0b1111_1111, 0b1110_0011, 0b1110_0000, 0b1100_0011];
        let x = FrameHeader::new(data);
        assert_eq!(x.unwrap().channel_mode, ChannelMode::SingleChannel);
    }
    /// Verifies that FrameHeader::new() correctly parses the mode extension for Layer I
    #[test]
    fn test_frame_header_new_mode_ext_layer1()
    {
        // Bands 4 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(4));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 8 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1110_0000, 0b0101_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(8));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 12 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1110_0000, 0b0110_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(12));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 16 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1111, 0b1110_0000, 0b0111_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(16));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);
    }
    /// Verifies that FrameHeader::new() correctly parses the mode extension for Layer II
    #[test]
    fn test_frame_header_new_mode_ext_layer2()
    {
        // Bands 4 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(4));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 8 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1110_0000, 0b0101_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(8));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 12 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1110_0000, 0b0110_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(12));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);

        // Bands 16 to 31
        let data: [u8; 4] = [0b1111_1111, 0b1111_1101, 0b1110_0000, 0b0111_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, Some(16));
        assert_eq!(x.intensity_stereo, None);
        assert_eq!(x.ms_stereo, None);
    }

    /// Verifies that FrameHeader::new() correctly parses the mode extension for Layer III
    #[test]
    fn test_frame_header_new_mode_ext_layer3()
    {
        // Intensity Stereo off, MS Stereo off
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, None);
        assert_eq!(x.intensity_stereo, Some(false));
        assert_eq!(x.ms_stereo, Some(false));

        // Intensity Stereo on, MS Stereo off
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0101_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, None);
        assert_eq!(x.intensity_stereo, Some(true));
        assert_eq!(x.ms_stereo, Some(false));

        // Intensity Stereo off, MS Stereo on
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0110_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, None);
        assert_eq!(x.intensity_stereo, Some(false));
        assert_eq!(x.ms_stereo, Some(true));

        // Intensity Stereo off, MS Stereo on
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0111_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.mode_ext_band, None);
        assert_eq!(x.intensity_stereo, Some(true));
        assert_eq!(x.ms_stereo, Some(true));
    }

    /// Verifies that FrameHeader::new() correctly parses the copyright flag
    #[test]
    fn test_frame_header_new_copyright()
    {
        // Without copyright
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.copy_righted, false);

        // With copyright
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_1011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.copy_righted, true);
    }

    /// Verifies that FrameHeader::new() correctly parses the original flag
    #[test]
    fn test_frame_header_new_original()
    {
        // Copy
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.original, false);

        // Original
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0111];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.original, true);
    }

    /// Verifies that FrameHeader::new() correctly parses the emphasis value
    #[test]
    fn test_frame_header_new_emphasis()
    {
        // No emphasis
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0000];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.emphasis, Emphasis::None);

        // 50/15 ms
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0001];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.emphasis, Emphasis::Ms5015);

        // CCIT J.17
        let data: [u8; 4] = [0b1111_1111, 0b1111_1011, 0b1110_0000, 0b0100_0011];
        let x = FrameHeader::new(data).unwrap();
        assert_eq!(x.emphasis, Emphasis::CcitJ17);
    }
    /// Verifies that FrameHeader::calc_frame_len() correctly calculates the length of a frame for a given frame header.
    #[test]
    fn test_frame_header_calc_frame_len()
    {
        // Version 1, Layer 3, 128Kbps, 44.1KHz sample rate, not padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer3,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 128_000,
            sample_rate: 44_100,
            padded: false,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 417);

        // Version 1, Layer 3, 128Kbps, 44.1KHz sample rate, padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer3,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 128_000,
            sample_rate: 44_100,
            padded: true,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 418);

        // Version 1, Layer 1, 128Kbps, 44.1KHz sample rate, not padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer1,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 128_000,
            sample_rate: 44_100,
            padded: false,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 139);

        // Version 1, Layer 1, 128Kbps, 44.1KHz sample rate, not padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer1,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 128_000,
            sample_rate: 44_100,
            padded: true,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 140);

        // Version 1, Layer 3, 448Kbps, 44.1KHz sample rate, not padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer3,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 448_000,
            sample_rate: 44_100,
            padded: false,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 1_462);

        // Version 1, Layer 3, 448Kbps, 44.1KHz sample rate, padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version1,
            layer_desc: LayerDesc::Layer3,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 448_000,
            sample_rate: 44_100,
            padded: true,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 1_463);

        // Version 2, Layer 3, 144Kbps, 44.1KHz sample rate, not padded
        let header = FrameHeader {
            mpeg_version: MpegVersion::Version2,
            layer_desc: LayerDesc::Layer3,
            protection_bit: ProtectionBit::Unprotected,
            bit_rate: 144_000,
            sample_rate: 44_100,
            padded: false,
            private: false,
            channel_mode: ChannelMode::SingleChannel,
            mode_ext_band: None,
            intensity_stereo: None,
            ms_stereo: None,
            copy_righted: true,
            original: false,
            emphasis: Emphasis::None,
        };
        assert_eq!(header.calc_frame_len(), 235);
    }
}