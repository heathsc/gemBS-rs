include ../gemBS_config.mk
.PHONY: ctoml all clean distclean

all: ctoml bin
	GEMBS_INSTALL_ROOT=$(GEMBS_INSTALL_ROOT) cargo build --release 
	cp target/release/gem_bs bin/gemBS

ifeq ($(SLURM_SUPPORT),yes)
ctoml: Cargo.toml.in
	cp Cargo.toml.in Cargo.toml
	echo default = [\"slurm\"] >> Cargo.toml
else
ctoml: Cargo.toml.in
	cp Cargo.toml.in Cargo.toml
endif

bin:
	mkdir bin

clean:
	rm -rf target Cargo.toml

distclean: clean
	rm -rf bin
