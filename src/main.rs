
use std::io;
use std::fs;
use std::process;
use std::path::PathBuf;
use std::error::Error;
use std::str::FromStr;

use bit_io::BitWriter;
use bit_io::ToBytes;
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Sample;

mod bit_io;
use bit_io::{BitReader, FromBytes};

#[derive(Parser, Debug, Clone)]
#[command(version, about="Playback raw audio samples.", long_about=None)]
struct Opt {
    /// Playback sample rate
    #[arg(short='r', long, default_value_t = 44_100)]
    sample_rate: u32,

    /// Size of samples in bits, supports: 8, 16, 32, 64
    #[arg(short='s', long, default_value_t = 32)]
    sample_size: u32,

    /// Number of channels in the audio stream
    #[arg(short, long, default_value_t = 2)]
    channels: u16,

    /// Loudness of the audio from 0.0 to 1.0
    ///
    /// By default, the output amplitude is reduced to 1/3rd
    ///
    /// --dangerous allows for this value to be set to higher than 1.0
    ///
    /// --loud disables the default output attenuation
    #[arg(short, long, default_value_t = 1.0)]
    gain: f32,

    /// Input samples are unsigned, incompatible with --float
    #[arg(short, long, default_value_t = false)]
    unsigned: bool,

    /// Input samples are floating point numbers, incompatible with <32 bit sample size
    #[arg(short, long, default_value_t = false)]
    float: bool,

    /// Input samples are big-endian, ignored with 8 bit samples
    #[arg(short, long="big-endian", default_value_t = false)]
    be: bool,

    /// Send post-process f32 values to stdout, incompatible with --pre
    #[arg(long="post", default_value_t = false)]
    post_out: bool,

    /// Send pre-process (configured input) values to stdout, incompatible with --post
    #[arg(long="pre", default_value_t = false)]
    pre_out: bool,

    /// Disables the signal input attenuation step
    /// By default, the output amplitude is reduced to 1/3rd
    #[arg(long, default_value_t = false)]
    loud: bool,

    /// Disables limits on gain (-g, --gain)
    #[arg(long, default_value_t = false)]
    dangerous: bool,

    /// Acknowledgement to use dangerous, unbounded features
    ///
    /// By enabling this option, you acknowledge the dangers associated with use or misuse of these features
    /// 
    /// Improper use of these features may lead to permanent hearing loss and/or damage of your speakers
    #[arg(long, default_value_t = false)]
    i_understand: bool,

    /// Input file path, if not specified, stdin will be used
    infile: Option<String>,
}

struct ValidConfigOut {
    sample_format: cpal::SampleFormat,
    sample_source: Box<dyn io::Read + Send>,
    sample_sink: Option<Box<dyn io::Write + Send>>
}

/// Sanity checks the sample format configuration, emits some errors.
/// Returns the sample format in the appropriate [cpal::SampleFormat] enum.
fn config_sanity_check(opt: &mut Opt) -> Result<ValidConfigOut, String> {
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

    match (opt.pre_out, opt.post_out) {
        (true, true) => {
            return Err("Incompatible options '--pre' and '--post', can choose only one or none".into());
        },
        _ => (),
    }

    let input: Box<dyn io::Read + Send> = if let Some(ref infile) = opt.infile {
        let path = PathBuf::from_str(&infile)
            .map_err(|e| format!("{e}"))?;

        let file = fs::File::options()
            .read(true)
            .write(false)
            .create(false)
            .open(path)
                .map_err(|e| format!("{e}"))?;

        let buffered_file = io::BufReader::new(file);
        Box::new(buffered_file)
    } else {
        let stdin = io::stdin();
        let buffered_stdin = io::BufReader::new(stdin);
        Box::new(buffered_stdin)
    };

    let output: Option<Box<dyn io::Write + Send>> = if opt.pre_out || opt.post_out {
        let stdout = io::stdout();
        Some(Box::new(stdout))
    } else {
        None
    };

    let is_using_dangerous_features = opt.dangerous || opt.loud;

    if opt.be && opt.sample_size == 8 {
        eprintln!("[!] endianness ignored (--be), irrelevant with 8-bit samples");
    }

    if opt.sample_rate < 8000 {
        eprintln!("[!] low sample rate (<8kHz), audio may be very distorted");
    }

    if opt.dangerous {
        eprintln!("[!] limits removed from gain input, may produce very loud sounds");
    } else {
        if !(0.0 <= opt.gain && opt.gain <= 1.0) {
            eprintln!("[!] invalid gain value {}, will be clamped between 0.0 and 1.0", opt.gain);
        }
    }

    if is_using_dangerous_features && !opt.i_understand {
        eprintln!("[!] LOUD SOUND WARNING: --dangerous and --loud may generate very loud sounds that could permanently damage your hearing and/or computer.");
        eprintln!("[!] To use these features, pass the --i-understand option to the program.");
        std::process::exit(1);
    }

    if !opt.dangerous {
        opt.gain = opt.gain.clamp(0.0, 1.0);
    }

    if !opt.loud {
        opt.gain = opt.gain.mul_amp(0.33);
    }

    Ok(ValidConfigOut {
        sample_format,
        sample_source: input,
        sample_sink: output,
    })
}

fn main() {
    let mut opt = Opt::parse();
    let result = config_sanity_check(&mut opt);
    if let Err(msg) = result {
        eprintln!("{msg}");
        process::exit(1);
    }
    let ValidConfigOut { sample_format, sample_source, sample_sink, } = result.unwrap();
    let input = sample_source;
    let output = sample_sink;

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
        cpal::SampleFormat::I8  => run::< i8>(&device, &oconfig.into(), opt, input, output),
        cpal::SampleFormat::U8  => run::< u8>(&device, &oconfig.into(), opt, input, output),

        cpal::SampleFormat::I16 => run::<i16>(&device, &oconfig.into(), opt, input, output),
        cpal::SampleFormat::U16 => run::<u16>(&device, &oconfig.into(), opt, input, output),

        cpal::SampleFormat::I32 => run::<i32>(&device, &oconfig.into(), opt, input, output),
        cpal::SampleFormat::U32 => run::<u32>(&device, &oconfig.into(), opt, input, output),

        cpal::SampleFormat::I64 => run::<i64>(&device, &oconfig.into(), opt, input, output),
        cpal::SampleFormat::U64 => run::<u64>(&device, &oconfig.into(), opt, input, output),

        cpal::SampleFormat::F32 => run::<f32>(&device, &oconfig.into(), opt, input, output),
        cpal::SampleFormat::F64 => run::<f64>(&device, &oconfig.into(), opt, input, output),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }.unwrap();
}

fn run<I>(
    device: &cpal::Device,
    oconfig: &cpal::StreamConfig,
    opt: Opt,
    input: Box<dyn io::Read + Send>,
    output: Option<Box<dyn io::Write + Send>>,
) -> Result<(), Box<dyn Error>> 
where 
  I: cpal::SizedSample + dasp_sample::ToSample<f32> + FromBytes + ToBytes {
    let mut bitreader = BitReader::new(input, opt.be);
    let mut bitwriter = None;
    if let Some(output) = output {
        bitwriter = Some(BitWriter::new(output, opt.be));
    }

    let mut next_sample = move || -> I {
        bitreader.read()
            .inspect_err(|_| process::exit(1))
            .unwrap()
    };

    let err_fn = move |err| {
        eprintln!("an error occurred on stream: {}", err)
    };

    let pre_out = opt.pre_out;
    let post_out = opt.post_out;
    let gain = opt.gain;
    let channels = oconfig.channels as usize;

    let stream = device.build_output_stream(
        &oconfig, 
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo|{
            write_data(
                data, channels, gain, 
                &mut next_sample, 
                pre_out, post_out, 
                &mut bitwriter,
            );
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
    pre_out: bool,
    post_out: bool,
    mut out_io: &mut Option<BitWriter<Box<dyn std::io::Write + Send>>>,
)
where
  I: cpal::SizedSample + dasp_sample::ToSample<f32> + ToBytes {
    for frame in output.chunks_mut(channels) {
        for sample in frame.iter_mut() {
            let pre_value = next_sample();
            let post_value = pre_value
                .to_sample::<f32>()
                .mul_amp(gain);

            match (&mut out_io, pre_out, post_out) {
                (Some(out_io), true, false) => {
                    out_io.write(pre_value).unwrap();
                },
                (Some(out_io), false, true) => {
                    out_io.write(post_value).unwrap();
                },
                (Some(_), true, true) => panic!("--pre and --post both enabled"),
                _ => (),
            }

            *sample = post_value;
        }
    }
}

