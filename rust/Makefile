include ../gemBS_config.mk
.PHONY: all clean distclean

GEMBS_INSTALL_ROOT ?= /usr/local/lib/gemBS
BINDIR=bin
GEMBSDIR=gemBS
GEMBS_CTOML=$(GEMBSDIR)/Cargo.toml

all: $(GEMBS_CTOML) $(BINDIR)
	GEMBS_INSTALL_ROOT=$(GEMBS_INSTALL_ROOT) RUSTFLAGS="-L $(TMP_ROOT)/lib" cargo build --release 
	cp target/release/gem_bs $(BINDIR)/gemBS
	cp target/release/read_filter $(BINDIR)/
	cp target/release/bs_call $(BINDIR)/
	cp target/release/snpxtr $(BINDIR)/
	cp target/release/dbsnp_index $(BINDIR)/

ifeq ($(SLURM_SUPPORT),yes)
$(GEMBS_CTOML): $(GEMBSDIR)/Cargo.toml.in
	cp $(GEMBSDIR)/Cargo.toml.in $(GEMBS_CTOML)
	echo default = [\"slurm\"] >> $(GEMBS_CTOML)
else
$(GEMBS_CTOML): $(GEMBSDIR)/Cargo.toml.in
	cp $(GEMBSDIR)/Cargo.toml.in $(GEMBS_CTOML)
endif

$(BINDIR):
	mkdir $(BINDIR)

clean:
	rm -rf target $(GEMBS_CTOML)

distclean: clean
	rm -rf $(BINDIR)
