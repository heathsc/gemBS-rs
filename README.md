# gemBS-rs
A rewrite of the [gemBS](https://github.com/heathsc/gemBS) pipeline
framework from Python/C into Rust.

gemBS is a high performance bioinformatic pipeline designed for highthroughput analysis
of DNA methylation data from whole genome bisulfites sequencing data
(WGBS). It combines GEM3, a high performance read aligner and
bs_call, a high performance variant and methyation caller, into a streamlined and efficient pipeline for
bisulfite sequence analysis.

The manuscript describing the original gemBS pipeline is available
[here](https://doi.org/10.1093/bioinformatics/bty690)

The rewrite of the pipeline into Rust has two aims: (1) to have a more
robust pipeline and (2) to provide a more flesible platform for future
develments.  At the moment the pipeline itself (previously in Python)
and the SNP/methylation caller bs_call (previously in C) have been
re-written in Rust.  It is planned to continue moving the other
binaries (mextr, snpxtr and dbSNP_idx) to Rust.  The GEM3 mapper,
being an external project (and also very stable!) will remain in C. 

---------   
Licensing
---------

gemBS is licensed under the GPL.

--------
Download
--------

Use ``git clone --recursive`` to retrieve the complete source code including the code from external projects such as ``gem3-mapper``.

    git clone --recursive https://github.com/heathsc/gemBS-rs.git

Before starting the installation of gemBS, you will need to install
or check the installation of several packages.

  a) gcc with development libraries
  
  b) rust (for installation instructions see [here](https://www.rust-lang.org/learn/get-started))
  
  c) zlib, libz2, lzma, openssl, libcurl, libncurses, wget, expat, ncurses, openssl, freetype, fontconfig
  
If you are working on a clean (fairly recent) Ubuntu installation, you
can install everything required with the following commands:

    apt-get install -y build-essential git autoconf wget lbzip2 pkg-config cmake
    apt-get install -y zlib1g-dev libbz2-dev libexpat1-dev
    apt-get install -y libncurses5-dev liblzma-dev libssl-dev libcurl4-openssl-dev curl
    apt-get install -y libfreetype6-dev libfontconfig1-dev
    curl https://sh.rustup.rs -sSf > rust.sh && sh rust.sh -y

Download the gemBS distribution if you haven't already done so:

    git clone --recursive https://github.com/heathsc/gemBS-rs.git
    
From the main gemBS-rs directory type the following to make the default config file:

    make gemBS_config.mk
    
Then look at the file gemBS_config.mk and make any changes that are required.  When the file is OK the pipeline and components can be built and installed by typing:

    make install


