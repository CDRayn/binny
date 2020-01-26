use std::{error::Error, fmt};

// https://www.datavoyage.com/mpgscript/mpeghdr.htm

// MPEG Audio version ID
enum MpegVersion
{
    Version25,  // MPEG Version 2.5 (00)
Reserved,   // Reserved bit combination (01)
Version2,   // MPEG Version 2 (10)
Version1,   // MPEG Version 1 (11)
}

// Layer Description
enum LayerDesc
{
    Reserved,   // Reserved bit combination (00)
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

// Bitrate
enum Bitrate
{
    Free = 0,
    Kbps8 = 8000,
    Kbps16 = 16_000,
    Kbps24 = 24_000,
    Kbps32 = 32_000,
    Kbps40 = 40_000,
    Kbps48 = 48_000,
    Kbps56 = 56_000,
    Kbps64 = 64_000,
    Kbps80 = 80_000,
    Kbps96 = 96_000,
    Kbps112 = 112_000,
    Kbps128 = 128_000,
    Kbps144 = 144_000,
    Kbps160 = 160_000,
    Kbps176 = 176_000,
    Kbps192 = 192_000,
    Kbps224 = 224_000,
    Kbps256 = 256_000,
    Kbps288 = 288_000,
    Kbps320 = 320_000,
    Kbps352 = 352_000,
    Kbps384 = 384_000,
    Kbps416 = 416_000,
    Kbps448 = 448_000,
}

// Sampling Rate Frequency
enum SampleFreq
{
    KHz8 = 8000,
    KHz11 = 11_025,
    KHz12 = 12_000,
    KHz16 = 16_000,
    KHz22 = 22_050,
    KHz24 = 24_000,
    KHz32 = 32_000,
    Khz44 = 44_100,
    KHz48 = 48_000,
}

// Channel Mode
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
    mpeg_version: MpegVersion,  // MPEG Version of the frame
    layer: LayerDesc,           // MPEG layer of the frame
    unprotected: bool,          // If true, no 16 bit CRC follows the header
    bit_rate: Bitrate,          // The bitrate for the frame
    sample_freq: SampleFreq,    // The sample rate of the frame in bits per second
    padded: bool,               // If true, use a padding slot to fit the bitrate
    private: bool,              // Informative only
    channel_mode: ChannelMode,  // Channel model of the frame
    mode_extension: u8,         // Only used in joint stereo and only values 0, 1, 2, & 3 are permitted
    copy_righted: bool,         // Has the same meaning as the copyright bit on CDs
    original: bool,             // If true, the frame presides on its original media
    emphasis: Emphasis,         // Tells the de-coder to de-emphasize the file during decoding, is rarely used
}

// TODO: Make errors more granular by specifying what is wrong in the header, rather than just specifying
//  that the header is invalid
// Error Invalid Headers
#[derive(Debug)]
struct FrameHeaderError;

impl Error for FrameHeader {}

impl fmt::Display for FrameHeaderError
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "Invalid frame header")
    }
}

// Represents an MP3 frame. Each frame contains a header struct and a vector of the bytes
// of the data portion of the frame.
struct Frame<'a>
{
    header: Result<&'a FrameHeader, FrameHeaderError>,
    data: Vec<u8>,
}

// Represents a parsed MP3 file as a sequence of repeating parsed MP3 frames
struct ParsedMp3
{
    frames: vec!(Frame),
    len: u32,
}

impl ParsedMp3


#[cfg(test)]
mod tests
{

}