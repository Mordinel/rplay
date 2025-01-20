use std::io;
use std::process;
use std::error::Error;

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;

mod bitreader;
use bitreader::{BitReader, FromBytes};

#[derive(Parser, Debug, Clone)]
#[command(version, about="Play raw audio samples from stdin", long_about=None)]
struct Opt {
    /// Playback sample rate.
    #[arg(short='r', long, default_value_t = 44_100)]
    sample_rate: u32,

    /// Size of samples, supports: 8, 16, 32, 64
    #[arg(short='s', long, default_value_t = 32)]
    sample_size: u32,

    /// Number of channels in the audio stream
    #[arg(short, long, default_value_t = 2)]
    channels: u16,

    /// Loudness of the audio from 0.0 to 1.0
    #[arg(short, long, default_value_t = 1.0)]
    gain: f32,

    /// Interpret integer samples as unsigned, incompatible with --float
    #[arg(short, long, default_value_t = false)]
    unsigned: bool,

    /// Read samples as floating point numbers, incompatible with <16 bit sample size
    #[arg(short, long, default_value_t = false)]
    float: bool,

    /// Read samples larger than 8 bits as big-endian, ignored with 8 bit samples
    #[arg(short, long, default_value_t = false)]
    be: bool,

    /// Suppress non-fatal errors
    #[arg(short, long, default_value_t = false)]
    quiet: bool,
}

/// Sanity checks the sample format configuration, emits some errors.
/// Returns the sample format in the appropriate [cpal::SampleFormat] enum.
fn config_sanity_check(opt: &mut Opt) -> Result<cpal::SampleFormat, String> {
    use cpal::SampleFormat::*;
    let sample_format = match (opt.float, opt.unsigned, opt.sample_size) {
        (false, false, 8) => I8,
        (false,  true, 8) => U8,

        (false, false, 16) => I16,
        (false,  true, 16) => U16,

        (false, false, 32) => I32,
        (false,  true, 32) => U32,

        (false, false, 64) => I64,
        (false,  true, 64) => U64,

        (true, false, 32) => F32,
        (true, false, 64) => F64,

        (true, true, _) => {
            return Err("Floating point values can not be represented as unsigned".into());
        },

        (true, false, invalid_size) => {
            return Err(format!("Unsupported floating point size: '{invalid_size}', can only be: [32, 64]"));
        },

        (false, _, invalid_size) => {
            return Err(format!("Unsupported sample size: '{invalid_size}'"));
        },
    };

    // non-fatal startup errors
    if !opt.quiet {
        if opt.be && opt.sample_size == 8 {
            eprintln!("[!] endianness ignored (--be), irrelevant with 8-bit samples");
        }

        if opt.sample_rate < 8000 {
            eprintln!("[!] low sample rate (<8kHz), audio may be very distorted");
        }

        if !(0.0 <= opt.gain && opt.gain <= 1.0) {
            eprintln!("[!] invalid gain value {}, will be clamped between 0.0 and 1.0", opt.gain);
        }
    }

    opt.gain = opt.gain.clamp(0.0, 1.0).mul_amp(0.1);

    Ok(sample_format)
}

fn main() {
    let mut opt = Opt::parse();
    let result = config_sanity_check(&mut opt);
    if let Err(msg) = result {
        eprintln!("{msg}");
        process::exit(1);
    }
    let sample_format = result.unwrap();

    let host = cpal::default_host();
    let device = host.default_output_device()
        .expect("failed to find output device");

    let channels = opt.channels;
    let sample_rate = cpal::SampleRate(opt.sample_rate);
    let buffer_size = cpal::SupportedBufferSize::Unknown;
    let iconfig_s = cpal::SupportedStreamConfig::new(
        channels,
        sample_rate,
        buffer_size,
        sample_format,
    );
    let iconfig = iconfig_s.config();

    let oconfig = device.default_output_config().unwrap();
    let oconfig = cpal::SupportedStreamConfig::new(
        iconfig.channels,
        iconfig.sample_rate,
        cpal::SupportedBufferSize::Unknown,
        oconfig.sample_format(),
    );

    let iformat = iconfig_s.sample_format();
    match iformat {
        cpal::SampleFormat::I8  => run::< i8>(&device, &oconfig.into(), opt),
        cpal::SampleFormat::U8  => run::< u8>(&device, &oconfig.into(), opt),

        cpal::SampleFormat::I16 => run::<i16>(&device, &oconfig.into(), opt),
        cpal::SampleFormat::U16 => run::<u16>(&device, &oconfig.into(), opt),

        cpal::SampleFormat::I32 => run::<i32>(&device, &oconfig.into(), opt),
        cpal::SampleFormat::U32 => run::<u32>(&device, &oconfig.into(), opt),

        cpal::SampleFormat::I64 => run::<i64>(&device, &oconfig.into(), opt),
        cpal::SampleFormat::U64 => run::<u64>(&device, &oconfig.into(), opt),

        cpal::SampleFormat::F32 => run::<f32>(&device, &oconfig.into(), opt),
        cpal::SampleFormat::F64 => run::<f64>(&device, &oconfig.into(), opt),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }.unwrap();
}

fn run<I>(
    device: &cpal::Device,
    oconfig: &cpal::StreamConfig,
    opt: Opt,
) -> Result<(), Box<dyn Error>> 
where 
  I: cpal::SizedSample + dasp_sample::ToSample<f32> + FromBytes {
    let stdin = io::stdin();
    let buffered_stdin = io::BufReader::new(stdin);
    let mut bitreader = BitReader::new(buffered_stdin, opt.be);

    let mut next_sample = move || -> I {
        bitreader.read()
            .inspect_err(|_| process::exit(1))
            .unwrap()
    };

    let quiet = opt.quiet;

    let err_fn = move |err| if !quiet {
        eprintln!("an error occurred on stream: {}", err)
    };

    let gain = opt.gain;
    let channels = oconfig.channels as usize;

    let stream = device.build_output_stream(
        &oconfig, 
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo|{
            write_data(data, channels, gain, &mut next_sample);
        },
        err_fn,
        None,
    )?;
    stream.play()?;

    std::thread::park();

    Ok(())
}

fn write_data<I>(
    output: &mut [f32],
    channels: usize,
    gain: f32,
    next_sample: &mut dyn FnMut() -> I,
)
where
  I: cpal::SizedSample + dasp_sample::ToSample<f32> {
    for frame in output.chunks_mut(channels) {
        for sample in frame.iter_mut() {
            let value = next_sample()
                .to_sample::<f32>()
                .mul_amp(gain)
                .clamp(-1.0, 1.0); // hard limiting
            *sample = value;
        }
    }
}

