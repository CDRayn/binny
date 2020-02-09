use std::{error::Error, fmt};
use std::io::Read;

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

// MPEG Audio version ID
#[derive(Clone, Copy, PartialEq)]
enum MpegVersion
{
    Version25,  // MPEG Version 2.5 (00)
    // Reserved bit combination (01)
    Version2,   // MPEG Version 2 (10)
    Version1,   // MPEG Version 1 (11)
}

// Layer Description
#[derive(Clone, Copy, PartialEq)]
enum LayerDesc
{
    // Reserved bit combination (00)
    Layer3,     // Layer III (01)
    Layer2,     // Layer II (10)
    Layer1,     // Layer I (11)
}

// Protection bit
enum ProtectionBit
{
    Protected, // Protected by following 16 bit CRC header (0)
    Unprotected, // Not protected (1)
}
// Channel Mode
#[derive(PartialEq)]
enum ChannelMode
{
    Stereo,
    JointStereo,    // Stereo
    DualChannel,    // 2 Mono Channels
    SingleChannel,  // Mono
}

enum Emphasis
{
    None,
    Ms5015,
    Reserved,
    CcitJ17,
}

// Audio Layer I/II/II frame header
struct FrameHeader
{
    mpeg_version: MpegVersion,      // MPEG Version of the frame
    layer_desc: LayerDesc,          // MPEG layer of the frame
    unprotected: ProtectionBit,     // If true, no 16 bit CRC follows the header
    bit_rate: u32,                  // The bitrate for the frame
    sample_rate: u32,               // The sample rate of the frame in bits per second
    padded: bool,                   // If true, use a padding slot to fit the bitrate
    private: bool,                  // Informative only
    channel_mode: ChannelMode,      // Channel model of the frame
    mode_ext_band: Option<u8>,      // Only used in Layer I & II joint stereo. The value is the start band.
    intensity_stereo: Option<bool>, // Only used in Layer III join stereo.
    ms_stereo: Option<bool>,        // Only used in Layer III join stereo.
    copy_righted: bool,             // Has the same meaning as the copyright bit on CDs
    original: bool,                 // If true, the frame presides on its original media
    emphasis: Emphasis,         // Tells the de-coder to de-emphasize the file during decoding, is rarely used
}

// TODO: Make errors more granular by specifying what is wrong in the header, rather than just specifying
//  that the header is invalid
// Error Invalid Headers
#[derive(Debug)]
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
            [160_000,   80_000,     64_000,     80_000,     40_00],
            [192_000,   96_000,     80_000,     96_000,     48_000],
            [224_000,   112_000,    96_000,     112_000,    56_000],
            [256_000,   128_000,    112_000,    128_000,    64_000],
            [228_000,   160_000,    128_000,    144_000,    80_000],
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

        let look_up = 0;
        match ver
        {
            MpegVersion::Version1 => {
                let look_up = match layer
                {
                    LayerDesc::Layer1 => 0,
                    LayerDesc::Layer2 => 1,
                    LayerDesc::Layer3 => 2,
                };
            }
            MpegVersion::Version2 | MpegVersion::Version25 => {
                let look_up = match layer
                {
                    LayerDesc::Layer1 => 3,
                    LayerDesc::Layer2 => 4,
                    LayerDesc::Layer3 => 4,
                };
            },
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
            MpegVersion::Version25 => 1,
        };
        return SAMPLING_RATES[bits as usize][look_up as usize];
    }

    // Accepts a slice of four u8 values and returns either FrameHeader or a FrameHeaderError
    // for invalid headers.
    fn new(slice: [u8; 4]) -> Result<FrameHeader, FrameHeaderError>
    {
        let value = u32::from_ne_bytes(slice);

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
            0b00 => FrameHeader::decode_sample_rate((SAMPLE_FREQ & value) >> 10, mpeg_version),
            _ => return Err(FrameHeaderError::new("Reserved value '0b11' used for sampling rate index!"))
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
        let mode_ext_band: Option<u8> = None;
        let intensity_stereo: Option<bool> = None;
        let ms_stereo: Option<bool> = None;

        if channel_mode == ChannelMode::JointStereo
        {
            if layer_desc == LayerDesc::Layer1 || layer_desc == LayerDesc::Layer2
            {
                let mode_ext_band = match (MODE_EXT & value) >> 4
                {
                    0b00 => Some(4),
                    0b01 => Some(8),
                    0b10 => Some(12),
                    0b11 => Some(16),
                    _    => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
                let intensity_stereo: Option<bool> = None;
                let ms_stereo: Option<bool> = None;
            }
            else
            {
                let mode_ext_band: Option<u8> = None;
                let intensity_stereo = match (MODE_EXT & value) >> 4
                {
                    0b00 => false,
                    0b01 => true,
                    0b10 => true,
                    0b11 => false,
                    _    => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
                let ms_stereo = match (MODE_EXT & value) >> 4
                {
                    0b00 => false,
                    0b01 => false,
                    0b10 => true,
                    0b11 => true,
                    _   => return Err(FrameHeaderError::new("Error encountered when parsing mode extension!"))
                };
            }
        }
        let copy_righted =  ((COPYRIGHT & value) >> 3) != 0;
        let original = ((ORIGINAL & value) >> 2) != 0;
        let emphasis = match (ORIGINAL & value) >> 2
        {
            0b00 => Emphasis::None,
            0b01 => Emphasis::Ms5015,
            0b10 => return Err(FrameHeaderError::new("Reserved value '0b10' used for emphasis!")),
            0b11 => Emphasis::CcitJ17,
            _ => return Err(FrameHeaderError::new("Error encountered when parsing emphasis!"))
        };

        return Ok(
            FrameHeader {
                mpeg_version,
                layer_desc,
                unprotected,
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