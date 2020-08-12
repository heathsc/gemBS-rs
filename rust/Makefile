include ../gemBS_config.mk
.PHONY: all clean distclean

all: bin
	GEMBS_INSTALL_ROOT=$(GEMBS_INSTALL_ROOT) cargo build --release 
	cp target/release/gem_bs bin/gemBS

bin:
	mkdir bin

clean:
	rm -rf target

distclean: clean
	rm -rf bin
