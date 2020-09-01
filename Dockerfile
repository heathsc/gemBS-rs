FROM ubuntu:xenial
MAINTAINER Simon Heath (simon.heath@gmail.com)
RUN apt-get update --fix-missing
RUN apt-get install -y build-essential git autoconf wget lbzip2 pkg-config cmake
RUN apt-get install -y zlib1g-dev libbz2-dev gsl-bin libgsl0-dev libexpat1-dev
RUN apt-get install -y libncurses5-dev liblzma-dev libssl-dev libcurl4-openssl-dev curl
RUN apt-get install -y libfreetype6-dev libfontconfig1-dev
RUN curl https://sh.rustup.rs -sSf > rust.sh && sh rust.sh -y
RUN mkdir /usr/local/build; cd /usr/local/build
RUN git clone --recursive https://github.com/heathsc/gemBS-rs.git
RUN (cd gemBS-rs; PATH=$PATH:/root/.cargo/bin GEMBS_CONTAINER=1 make install)
RUN rm -rf /usr/local/build
RUN echo "cd /home;export PATH=/usr/local/lib/gemBS/bin:/usr/local/lib/gemBS/texlive/bin/x86_64-linux:$PATH;gemBS \$@" > start.sh
ENTRYPOINT ["/bin/bash", "start.sh"]
