.PHONY: all clean distclean texlive

all: gemBS_config.mk
	$(MAKE) -f Makefile.gemBS

install: gemBS_config.mk
	$(MAKE) -f Makefile.gemBS install

gemBS_config.mk: gemBS_config.mk.in
	cp $< $@

texlive:
	$(MAKE) -f Makefile.gemBS texlive

clean:
	$(MAKE) -f Makefile.gemBS clean
	rm -f *~

distclean:
	$(MAKE) -f Makefile.gemBS distclean
	rm -f gemBS_config.mk *~
