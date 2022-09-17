#============================================================================
# PROJECT: gemBS
# FILE: Makefile
# DATE: 27/02/2021
# AUTHOR(S): Simon Heath <simon.heath@gmail.com>
# DESCRIPTION: Top level makefile
#============================================================================

# Definitions
ROOT_PATH=$(CURDIR)
GEMBS_INSTALL_ROOT ?= /usr/local/lib/gemBS
TMP_ROOT ?= $(abspath tmp_root)

# samtools and bcftools definitions
SAMTOOLS_VERSION=1.16.1
BCFTOOLS_VERSION=1.16
HTSLIB_VERSION=1.16
SAMTOOLS_DIR=samtools
BCFTOOLS_DIR=bcftools
HTSLIB_DIR=htslib
SAMTOOLS=$(SAMTOOLS_DIR)/samtools
BCFTOOLS=$(BCFTOOLS_DIR)/bcftools
SAMTOOLS_TAR=samtools.tar.bz2
BCFTOOLS_TAR=bcftools.tar.bz2
HTSLIB_TAR=htslib.tar.bz2
SAMTOOLS_URL=https://github.com/samtools/samtools/releases/download/$(SAMTOOLS_VERSION)/samtools-$(SAMTOOLS_VERSION).tar.bz2
BCFTOOLS_URL=https://github.com/samtools/bcftools/releases/download/$(BCFTOOLS_VERSION)/bcftools-$(BCFTOOLS_VERSION).tar.bz2
HTSLIB_URL=https://github.com/samtools/htslib/releases/download/$(HTSLIB_VERSION)/htslib-$(HTSLIB_VERSION).tar.bz2
HTSLIB_INSTALL=$(TMP_ROOT)/include/htslib/hts.h

MACHTYPE:=$(shell uname -m)
ifneq (,$(findstring -,$(MACHTYPE)))
    MACHTYPE:=$(shell uname -m)
endif
		  
FOLDER_BIN=bin

all: setup gem3 $(HTSLIB_INSTALL) _samtools _bcftools

all_debug: setup gem3-debug $(HTSLIB_INSTALL) _samtools _bcftools

$(HTSLIB_INSTALL): $(HTSLIB_DIR) $(HTSLIB_DIR)/config.mk $(HTSLIB_DIR)/Makefile.bgzip-shared
	$(MAKE) --directory=$(HTSLIB_DIR) && $(MAKE) --directory=$(HTSLIB_DIR) install
	$(MAKE) --directory=$(HTSLIB_DIR) -f Makefile.bgzip-shared bgzip-shared && cp $(HTSLIB_DIR)/bgzip-shared $(TMP_ROOT)/bin/bgzip

_samtools: $(HTSLIB_INSTALL) $(SAMTOOLS_DIR) $(SAMTOOLS_DIR)/config.mk 
	$(MAKE) --directory=$(SAMTOOLS_DIR) all

_bcftools: $(HTSLIB_INSTALL) $(BCFTOOLS_DIR) $(BCFTOOLS_DIR)/config.h
	$(MAKE) $(FOLDER_BIN)/bcftools

setup:
	@mkdir -p $(FOLDER_BIN) $(TMP_ROOT) $(TMP_ROOT)/bin

gem3: gem3-mapper/Makefile.mk
	$(MAKE) --directory=gem3-mapper
	
gem3-static: gem3-mapper/Makefile.mk
	$(MAKE) --directory=gem3-mapper static
	
gem3-debug: gem3-mapper/Makefile.mk
	$(MAKE) --directory=gem3-mapper debug

gem3-mapper/configure: gem3-mapper/configure.ac
	cd gem3-mapper; autoconf

gem3-mapper/Makefile.mk: gem3-mapper/Makefile.mk.in gem3-mapper/configure
	cd gem3-mapper; ./configure ${GEM3_CONFIG}

$(HTSLIB_DIR)/config.mk:
	cd $(HTSLIB_DIR); ./configure --prefix=$(TMP_ROOT)

$(HTSLIB_DIR)/Makefile.bgzip-shared:
	cd $(HTSLIB_DIR); ln -s ../Makefile.bgzip-shared .

$(SAMTOOLS_DIR)/config.mk:
	cd $(SAMTOOLS_DIR); ./configure --prefix=$(GEMBS_INSTALL_ROOT) --with-htslib=$(TMP_ROOT)

$(FOLDER_BIN)/bcftools: $(BCFTOOLS_DIR) 
	$(MAKE) --directory=$(BCFTOOLS_DIR) all

$(BCFTOOLS_DIR)/config.h:
	cd $(BCFTOOLS_DIR); ./configure --prefix=$(GEMBS_INSTALL_ROOT) --with-htslib=$(TMP_ROOT)
	touch $(BCFTOOLS_DIR)/config.h

$(HTSLIB_DIR):
	wget -O $(HTSLIB_TAR) $(HTSLIB_URL) && tar -jxf $(HTSLIB_TAR) && rm -f $(HTSLIB_TAR)
	mv htslib-$(HTSLIB_VERSION) $(HTSLIB_DIR)
#	(cd $(HTSLIB_DIR) && autoheader && autoconf)

$(SAMTOOLS_DIR):
	wget -O $(SAMTOOLS_TAR) $(SAMTOOLS_URL) && tar -jxf $(SAMTOOLS_TAR) && rm -f $(SAMTOOLS_TAR)
	mv samtools-$(SAMTOOLS_VERSION) $(SAMTOOLS_DIR)
#	(cd $(SAMTOOLS_DIR) && autoreconf)

$(BCFTOOLS_DIR): 
	wget -O $(BCFTOOLS_TAR) $(BCFTOOLS_URL) && tar -jxf $(BCFTOOLS_TAR) && rm -f $(BCFTOOLS_TAR)
	mv bcftools-$(BCFTOOLS_VERSION) $(BCFTOOLS_DIR)
#	(cd $(BCFTOOLS_DIR) && autoheader && autoconf)

clean:
	@rm -f *~
	@rm -rf $(FOLDER_BIN) $(TMP_ROOT)
	if [ -f $(SAMTOOLS_DIR)/Makefile ]; then $(MAKE) --directory=$(SAMTOOLS_DIR) clean; fi
	if [ -f $(BCFTOOLS_DIR)/Makefile ]; then $(MAKE) --directory=$(BCFTOOLS_DIR) clean; fi
	if [ -f "gem3-mapper/Makefile.mk" ]; then $(MAKE) --directory=gem3-mapper clean; fi

distclean:
	@rm -f *~
	@rm -rf $(FOLDER_BIN) $(TMP_ROOT)
	if [ -f $(SAMTOOLS_DIR)/Makefile ]; then cd $(SAMTOOLS_DIR); $(MAKE) clean; rm -f config.h config.log config.status config.mk; fi
	if [ -f $(BCFTOOLS_DIR)/Makefile ]; then cd $(BCFTOOLS_DIR); $(MAKE) clean; rm -f config.h config.log config.status config.mk; fi
	if [ -f $(HTSLIB_DIR)/Makefile ]; then cd $(HTSLIB_DIR); $(MAKE) clean; rm -f config.h config.log config.status config.mk; fi
	if [ -f "gem3-mapper/Makefile.mk" ]; then $(MAKE) --directory=gem3-mapper distclean; fi
