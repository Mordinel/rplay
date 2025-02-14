# rplay

Playback raw audio samples.
    
    Usage: rplay [OPTIONS] [INFILE]

    Arguments:
      [INFILE]  Input file path, if not specified, stdin will be used

    Options:
      -r, --sample-rate <SAMPLE_RATE>  Playback sample rate [default: 44100]
      -s, --sample-size <SAMPLE_SIZE>  Size of samples in bits, supports: 8, 16, 32, 64 [default: 32]
      -c, --channels <CHANNELS>        Number of channels in the audio stream [default: 2]
      -g, --gain <GAIN>                Loudness of the audio from 0.0 to 1.0 [default: 1]
      -u, --unsigned                   Input samples are unsigned, incompatible with --float
      -f, --float                      Input samples are floating point numbers, incompatible with <32 bit sample size
      -b, --big-endian                 Input samples are big-endian, ignored with 8 bit samples
          --post                       Send post-process f32 values to stdout, incompatible with --pre
          --pre                        Send pre-process (configured input) values to stdout, incompatible with --post
          --dangerous                  Disables limits on gain (-g, --gain)
      -h, --help                       Print help (see more with '--help')
      -V, --version                    Print version

Don't hurt your ears.
