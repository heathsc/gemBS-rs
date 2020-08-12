BootStrap: docker
From: ubuntu:xenial

%runscript
    exec /usr/local/bin/gemBS $@

%help
    gemBS singularity container
	 
%post
	 (mkdir /ext && cd /ext && mkdir disk1 disk2 disk3 disk4 disk5 disk6 disk7 disk8 disk9)
    apt-get update
    apt-get install -y build-essential git autoconf wget lbzip2 pkg-config cmake
    apt-get install -y zlib1g-dev libbz2-dev gsl-bin libgsl0-dev libexpat1-dev
    apt-get install -y libncurses5-dev liblzma-dev libssl-dev libcurl4-openssl-dev curl
    curl https://sh.rustup.rs -sSf > rust.sh && sh rust.sh -y
    mkdir -p /usr/local/build; cd /usr/local/build
    wget https://download.savannah.gnu.org/releases/freetype/freetype-2.10.0.tar.bz2 && tar -jxf freetype-2.10.0.tar.bz2
    (cd freetype-2.10.0 && ./configure --prefix=/usr && make -j8 && make install)
    git clone --recursive https://github.com/heathsc/gemBS-rs.git
    (cd gemBS-rs; PATH=$PATH:/root/.cargo/bin make install)
    (cd /usr/local/bin; ln -s /usr/local/lib/gemBS/bin/* .)
    rm -rf /usr/local/build
