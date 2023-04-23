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
robust pipeline and (2) to provide a more flexible platform for future
developments.  All of the tools developed for the pipeline except for the GEM3 mapper (being an external project that is also very stable!) have now been re-written in Rust. These include bs_call, the methylation and SNV-variant caller, and the methylation and SNP extractions tools mextr and snpxtr.  In all cases the running times are comparable to the original C versions.

------------
Dependencies
------------

The pipeline uses samtools for generating sorted BAM/CRAM files from GEM3 and bcftools for merging and indexing BCF files produced by bs_call.  In addition, many of the tools link to htslb to enable reading of BAM/CRAM and reading/writing of BCF files.  Samtools and htslib are automatically installed during the installation of the gemBS pipeline.   There is also an optional dependency on TeXLive which is used to produce pdf versions of the QC reports.  If requested by the user this is also installed with the pipeline.

---------   
Licensing
---------

gemBS is licensed under the GPL.

--------
Download
--------

Use ``git clone --recursive`` to retrieve the complete source code including the code from external projects such as ``gem3-mapper``.

    git clone --recursive https://github.com/heathsc/gemBS-rs.git

-------------------
Configure & Install
-------------------

Before starting the installation of gemBS, you will need to install
or check the installation of several packages.

  a) gcc with development libraries
  
  b) rust (for installation instructions see [here](https://www.rust-lang.org/learn/get-started)).  Note that if you have rust already installed you should update it using ``rustup update`` before trying to compile gemBS.
  
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

If the make and install process is successful, a shell script called gemBS will be created in the main gemBS-rs directory.  This file should be copied to a directory that is in your PATH so that gemBS can be invoked from anywhere.

-----------------------
Check your installation
-----------------------

For checking your installation follow this
[worked example](http://statgen.cnag.cat/gemBS/UserGuide/_build/html/example.html).

-------------
Documentation
-------------

Documentation can be found at
[gemBS](http://statgen.cnag.cat/gemBS/).

----------
Changelog:
----------
    4.1.5 Ensure that all user supplied options are passed on to slurm scripts
    4.1.4 Workaround for bug in plotters crate which could cause the
          generation of insertion size historgrams to hang
    4.1.3 Fix bug with generation of gemBS reference.  Fix bug with
          command lines options and slurm. Switch to samtools/htslib/bcftools 1.17
    4.1.2 Switch to clap v4.  Switch to samtools/htslib/bcftools 1.16
    4.1.1 Fix bug in cli for index sub command
    4.1.0 Switch to clap v3
    4.0.4 Add max_template_length option to gemBS (option passed on to bs_call)
    4.0.3 Fix problem with reading BCF files from older versions of
          gemBS where the CX format string was null terminated
    4.0.2 Change way we iterate over SAM/BAM/CRAM files to the same way used in samtools 
          view (the old way did not always work with cram files)
    4.0.2 Move to versions 1.12 of htslib/samtools/bcftools
    4.0.1 Correct bug preventing that caused non-stranded mapping to fail
    4.0 First release of gemBS-rs (4th release of gemBS)
