\NeedsTeXFormat{LaTeX2e}[1995/12/01]
\ProvidesClass{sphinxmanual}[2009/06/02 Document class (Sphinx manual)]

% chapters starting at odd pages (overridden by 'openany' document option)
\PassOptionsToClass{openright}{\sphinxdocclass}

% 'oneside' option overriding the 'twoside' default
\newif\if@oneside
\DeclareOption{oneside}{\@onesidetrue}
% Pass remaining document options to the parent class.
\DeclareOption*{\PassOptionsToClass{\CurrentOption}{\sphinxdocclass}}
\ProcessOptions\relax

% Defaults two-side document
\if@oneside
% nothing to do (oneside is the default)
\else
\PassOptionsToClass{twoside}{\sphinxdocclass}
\fi

\LoadClass{\sphinxdocclass}

% Set some sane defaults for section numbering depth and TOC depth.  You can
% reset these counters in your preamble.
%
\setcounter{secnumdepth}{2}
\setcounter{tocdepth}{1}

\usepackage{datetime}
\renewcommand{\dateseparator}{-}
\ddmmyyyydate

% Change the title page to look a bit better, and fit in with the fncychap
% ``Bjarne'' style a bit better.

\renewcommand{\maketitle}{%
  \providecommand{\finishdate}{\today}
  \begin{titlepage}%
    \let\footnotesize\small
    \let\footnoterule\relax
    \ifsphinxpdfoutput
      \begingroup
      % These \defs are required to deal with multi-line authors; it
      % changes \\ to ', ' (comma-space), making it pass muster for
      % generating document info in the PDF file.
      \def\\{, }
      \def\and{and }
      \pdfinfo{
        /Author (\@author)
        /Title (\@title)
      }
      \endgroup
    \fi
    \begin{flushright}%
     \begin{center}

       \begin{tabular*}{\textwidth}{@{}l@{\extracolsep{\fill}}r@{}}
        \bfseries{\@title}
        \end{tabular*}

        \rule{\textwidth}{1pt} \\[0.8 cm]

         \begin{tabular*}{\textwidth}{l@{} l@{}}
           \emph{Project:\quad } &  \projectname \\ 
           \emph{Sample(s):\quad } &
			  \parbox{0.8\linewidth}{\raggedright\samples} \\
			  \ifdefined\finishdate
             \emph{Date of report: \quad } & \finishdate \\
			  \fi
			  \ifdefined\startdate
			    \ifdefined\finishdate
			      & The analyses were performed between \startdate\ and \finishdate \\
			    \fi
			  \fi
         \end{tabular*}


         \rule{\textwidth}{1pt} \\[0.8 cm]

         \ifdefined\analyst
           \begin{tabular*}{\textwidth}{@{}l@{\extracolsep{\fill}}r@{}}
             \emph{Data Analyst:} & \emph{Authorized Signature:} \\ 
			    \analyst
           \end{tabular*}
			\fi

     \end{center}
    \end{flushright}%\par

    \thispagestyle{plain}%
    \setcounter{page}{1}

  \end{titlepage}%
  \cleardoublepage%
  \let\thanks\relax\let\maketitle\relax
}

% Catch the end of the {abstract} environment, but here make sure the abstract
% is followed by a blank page if the 'openright' option is used.
%
\let\py@OldEndAbstract=\endabstract
\renewcommand{\endabstract}{
  \if@openright
    \ifodd\value{page}
      \typeout{Adding blank page after the abstract.}
      \vfil\pagebreak
    \fi
  \fi
  \py@OldEndAbstract
}

% This wraps the \tableofcontents macro with all the magic to get the spacing
% right and have the right number of pages if the 'openright' option has been
% used.  This eliminates a fair amount of crud in the individual document files.
%
\let\py@OldTableofcontents=\tableofcontents
\renewcommand{\tableofcontents}{%
  \setcounter{page}{2}%
  \pagebreak%
  \pagestyle{plain}%
  {%
    \parskip = 0mm%
    \py@OldTableofcontents%
    \if@openright%
      \ifodd\value{page}%
        \typeout{Adding blank page after the table of contents.}%
        \pagebreak\hspace{0pt}%
      \fi%
    \fi%
    \cleardoublepage%
  }%
  \pagenumbering{arabic}%
  \@ifundefined{fancyhf}{}{\pagestyle{normal}}%
}

% This is needed to get the width of the section # area wide enough in the
% library reference.  Doing it here keeps it the same for all the manuals.
%
\renewcommand*\l@section{\@dottedtocline{1}{1.5em}{2.6em}}
\renewcommand*\l@subsection{\@dottedtocline{2}{4.1em}{3.5em}}

% Fix the bibliography environment to add an entry to the Table of
% Contents.
% For a report document class this environment is a chapter.
\let\py@OldThebibliography=\thebibliography
\renewcommand{\thebibliography}[1]{
  \cleardoublepage
  \phantomsection
  \py@OldThebibliography{1}
  \addcontentsline{toc}{chapter}{\bibname}
}
