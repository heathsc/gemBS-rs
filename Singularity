BootStrap: docker
From: ubuntu:xenial
Stage: build

%post
    apt-get update
    apt-get install -y build-essential git autoconf wget lbzip2 pkg-config cmake
    apt-get install -y zlib1g-dev libbz2-dev libexpat1-dev
    apt-get install -y libncurses5-dev liblzma-dev libssl-dev libcurl4-openssl-dev curl
    apt-get install -y libfreetype6-dev libfontconfig1-dev
    curl https://sh.rustup.rs -sSf > rust.sh && sh rust.sh -y
    mkdir -p /usr/local/build; cd /usr/local/build
    git clone --recursive https://github.com/heathsc/gemBS-rs.git
    (cd gemBS-rs; PATH=$PATH:/root/.cargo/bin GEMBS_CONTAINER=1 make install)
	 cd;  rm -rf /usr/local/build ~/.cargo
    echo /usr/local/lib/gemBS/lib > /etc/ld.so.conf.d/gemBS.conf && ldconfig

%runscript
    exec gemBS $@

%environment
    export PATH=/usr/local/lib/gemBS/bin:/usr/local/lib/gemBS/texlive/bin/x86_64-linux:$PATH

%help
    gemBS singularity container
