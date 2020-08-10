include gemBS_config.mk

GEMBS_INSTALL_ROOT ?= /usr/local/lib/gemBS
bin_dir = $(GEMBS_INSTALL_ROOT)/bin
etc_dir = $(GEMBS_INSTALL_ROOT)/etc
css_dir = $(etc_dir)/css
cfg_dir = $(etc_dir)/config_scripts

SUBDIRS = rust c_tools
INSTALLDIRS = $(GEMBS_INSTALL_ROOT) $(bin_dir) $(etc_dir) $(css_dir) $(cfg_dir)

.PHONY: subdirs $(SUBDIRS) installdirs all clean

all: subdirs

subdirs: $(SUBDIRS)

$(SUBDIRS): 
	$(MAKE) -C $@

install: all installdirs
	cp rust/bin/* $(bin_dir)
	cp c_tools/bin/* $(bin_dir)
	cp c_tools/bcftools/bcftools $(bin_dir)
	cp c_tools/samtools/samtools $(bin_dir)
	cp c_tools/htslib/bgzip $(bin_dir)
	cp c_tools/bs_call/bin/* $(bin_dir)
	cp c_tools/gem3-mapper/bin/gem-mapper $(bin_dir)
	cp c_tools/gem3-mapper/bin/gem-indexer $(bin_dir)
	cp etc/css/style.css $(etc_dir)
	cp etc/config_scripts/*.conf $(cfg_dir)

installdirs: $(INSTALLDIRS)

$(INSTALLDIRS):
	mkdir -p $@

$(bin_dir): $(GEMBS_INSTALL_ROOT)
$(etc_dir): $(GEMBS_INSTALL_ROOT)
$(css_dir): $(etc_dir)
$(cfg_dir): $(etc_dir)
