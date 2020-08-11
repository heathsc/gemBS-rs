PKGS = install latex-bin latexmk ec colortbl titlesec latex-tools-dev xcolor \
geometry hyperref pdftexcmds infwarerr kvoptions epstopdf-pkg

wget http://mirror.ctan.org/systems/texlive/tlnet/install-tl-unx.tar.gz
tar zxf install-tl-unx.tar.gz && rm install-tl-unx.tar.gz
cd install-tl-*
./install-tl -profile ../gembs_texlive.profile
/usr/local/texlive/bin/*/tlmgr install $(PKGS)
/usr/local/texlive/bin/*/tlmgr path add
